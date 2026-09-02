//! Android entry point.
//!
//! Roadmap step 6: mirrors `app-desktop`'s screen flow (main menu ->
//! multiplayer / texture picker -> in-game + HUD) and reuses the exact
//! same `client_core`/`renderer`/`ui`/`protocol`/`texturepacks` crates,
//! but drives it from touch input instead of keyboard/mouse:
//! `ui::TouchController` gives a virtual joystick (bottom-left) for
//! movement, a drag-anywhere-else look, and jump/mine/place buttons
//! (bottom-right); tapping during a menu screen is treated as a click
//! on whatever's under it, so the same `ui::screens::Screen` state
//! machine drives both platforms.
//!
//! Built via `cargo apk build` (see the repo README) into an `.apk`
//! targeting `[package.metadata.android]` below. This was written and
//! compiled against the `aarch64-linux-android` target in development,
//! but never run on a real device or emulator — none was available —
//! so treat the touch/IME handling as reviewed, not verified.

use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::{Camera, Chunk, ChunkColumn, PlayerPhysics, World};
use glam::Vec3;
use protocol::{GameEvent, OutgoingPosition};
use renderer::{mesh_world, GpuState};
use texturepacks::PickerEvent;
use ui::screens::{draw_hud, Action, PickerStatus, Screen, TexturePackPickerScreen};
use ui::{Font, Painter, TouchController, UiInput};
use winit::event::{Event, Touch, TouchPhase, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

const LOOK_SENSITIVITY: f32 = 0.006;
const POSITION_UPDATE_INTERVAL: Duration = Duration::from_millis(100);
const FONT_PIXEL_SIZE: f32 = 20.0; // a bit larger than desktop's 16px — easier to read at arm's length

struct Session {
    world: World,
    physics: PlayerPhysics,
    world_dirty: bool,
    net: Option<protocol::NetworkHandle>,
    ready: bool,
    last_position_sent: Instant,
}

impl Session {
    fn demo() -> Self {
        let mut column = ChunkColumn::empty(0, 0);
        column.set_section(0, Chunk::hardcoded_demo());
        let mut world = World::new();
        world.insert_column(column);

        Self {
            world,
            physics: PlayerPhysics::new(Vec3::new(8.0, 20.0, 8.0)),
            world_dirty: true,
            net: None,
            ready: true,
            last_position_sent: Instant::now(),
        }
    }

    fn networked(net: protocol::NetworkHandle) -> Self {
        Self {
            world: World::new(),
            physics: PlayerPhysics::new(Vec3::new(0.0, 80.0, 0.0)),
            world_dirty: false,
            net: Some(net),
            ready: false,
            last_position_sent: Instant::now(),
        }
    }

    fn drain_network_events(&mut self) {
        let Some(net) = &mut self.net else { return };
        let mut disconnected = false;
        loop {
            match net.events.try_recv() {
                Ok(GameEvent::JoinGame { dimension, .. }) => {
                    log::info!("joined game in dimension {dimension}");
                }
                Ok(GameEvent::ChunkColumnLoaded(column)) => {
                    self.world.insert_column(column);
                    self.world_dirty = true;
                }
                Ok(GameEvent::ChunkColumnUnloaded { chunk_x, chunk_z }) => {
                    self.world.remove_column(chunk_x, chunk_z);
                    self.world_dirty = true;
                }
                Ok(GameEvent::PlayerPositionAndLook { x, y, z, .. }) => {
                    self.physics.position = Vec3::new(x as f32, y as f32, z as f32);
                    self.physics.velocity = Vec3::ZERO;
                    self.ready = true;
                    log::info!("spawned at ({x:.1}, {y:.1}, {z:.1})");
                }
                Ok(GameEvent::BlockChange { x, y, z, block }) => {
                    self.world.set_block(x, y, z, block);
                    self.world_dirty = true;
                }
                Ok(GameEvent::ChatMessage(json)) => {
                    log::info!("chat: {json}");
                }
                Ok(GameEvent::Disconnected(reason)) => {
                    log::warn!("disconnected: {reason}");
                    disconnected = true;
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
                | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
        if disconnected {
            self.net = None;
        }
    }
}

enum Mode {
    Ui(Screen),
    InGame,
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));

    let event_loop = winit::event_loop::EventLoopBuilder::new()
        .with_android_app(app)
        .build()
        .expect("failed to create event loop");
    run(event_loop);
}

/// Non-Android builds (`cargo build -p app-android` on a desktop host)
/// only exist so this crate's logic gets checked by the normal
/// workspace build/clippy/test pipeline — see the README for why a
/// real device build goes through `cargo apk` instead.
#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn desktop_smoke_entry() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    run(event_loop);
}

fn run(event_loop: EventLoop<()>) {
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Cobble")
            .build(&event_loop)
            .expect("failed to create window"),
    );

    let mut gpu = pollster::block_on(GpuState::new(window.clone()));
    let aspect = gpu.size.0 as f32 / gpu.size.1 as f32;
    let mut camera = Camera::new(Vec3::new(8.0, 20.0, 8.0), aspect);

    let mut current_atlas = texturepacks::build_fallback_atlas();
    gpu.set_atlas_texture(&current_atlas);

    let ui_font = Font::load_regular(FONT_PIXEL_SIZE);
    gpu.set_ui_texture(&ui_font.atlas);

    let mut session: Option<Session> = None;
    let mut mode = Mode::Ui(Screen::MainMenu);

    let mut ui_input = UiInput::default();
    let mut touch = TouchController::new((gpu.size.0 as f32, gpu.size.1 as f32));
    let mut last_frame = Instant::now();
    let mut picker_events: Option<tokio::sync::mpsc::UnboundedReceiver<PickerEvent>> = None;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    gpu.resize((size.width, size.height));
                    touch.relayout((size.width as f32, size.height as f32));
                }
                WindowEvent::Touch(Touch { phase, location, id, .. }) => {
                    let pos = (location.x as f32, location.y as f32);
                    match &mode {
                        Mode::Ui(_) => {
                            if phase == TouchPhase::Started {
                                ui_input.mouse_pos = pos;
                                ui_input.clicked = true;
                            }
                        }
                        Mode::InGame => match phase {
                            TouchPhase::Started => touch.touch_down(id, pos),
                            TouchPhase::Moved => touch.touch_moved(id, pos),
                            TouchPhase::Ended | TouchPhase::Cancelled => touch.touch_up(id),
                        },
                    }
                }
                WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                    if let Mode::Ui(_) = &mode {
                        ui_input.text_input.push_str(&text);
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let dt = (now - last_frame).as_secs_f32();
                    last_frame = now;
                    let viewport = (gpu.size.0 as f32, gpu.size.1 as f32);

                    match &mut mode {
                        Mode::Ui(screen) => {
                            poll_picker_events(&mut picker_events, screen, &mut gpu, &mut current_atlas, &mut session);

                            let frame_input = ui_input.take();
                            let action = screen.update(&frame_input, viewport);

                            let mut painter = Painter::new(&ui_font);
                            screen.draw(&mut painter, viewport, frame_input.mouse_pos);
                            gpu.set_ui_draw_list(&painter.list);

                            apply_menu_action(action, &mut mode, &mut session, &mut picker_events, &mut touch, viewport, elwt);
                        }
                        Mode::InGame => {
                            let Some(active_session) = &mut session else {
                                mode = Mode::Ui(Screen::MainMenu);
                                return;
                            };

                            active_session.drain_network_events();
                            if active_session.world_dirty {
                                let (vertices, indices) = mesh_world(&active_session.world, &current_atlas);
                                if !indices.is_empty() {
                                    gpu.set_chunk_mesh(&vertices, &indices);
                                }
                                active_session.world_dirty = false;
                            }

                            let (dx, dy) = touch.take_look_delta();
                            camera.rotate(dx * LOOK_SENSITIVITY, -dy * LOOK_SENSITIVITY);

                            if active_session.ready {
                                let forward = camera.forward();
                                let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
                                let right = camera.right();
                                let (joy_x, joy_y) = touch.movement();
                                // Joystick y is screen-space (pulled down = positive),
                                // so "up"/forward on the stick is negative y.
                                let wish_dir = right * joy_x - flat_forward * joy_y;

                                let physics_dt = dt.min(1.0 / 20.0);
                                active_session.physics.update(&active_session.world, wish_dir, touch.jump.is_held(), false, physics_dt);
                            }
                            camera.position = active_session.physics.eye_position();

                            if let Some(net) = &active_session.net {
                                if active_session.ready && now.duration_since(active_session.last_position_sent) >= POSITION_UPDATE_INTERVAL {
                                    active_session.last_position_sent = now;
                                    let yaw_deg = camera.yaw.to_degrees() + 90.0;
                                    let pitch_deg = -camera.pitch.to_degrees();
                                    net.send_position(OutgoingPosition {
                                        x: active_session.physics.position.x as f64,
                                        y: active_session.physics.position.y as f64,
                                        z: active_session.physics.position.z as f64,
                                        yaw: yaw_deg,
                                        pitch: pitch_deg,
                                        on_ground: active_session.physics.on_ground,
                                    });
                                }
                            }

                            gpu.update_camera(camera.view_proj_matrix());

                            let mut painter = Painter::new(&ui_font);
                            draw_hud(&mut painter, viewport);
                            touch.draw(&mut painter);
                            gpu.set_ui_draw_list(&painter.list);
                        }
                    }

                    match gpu.render() {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => gpu.resize(gpu.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => log::warn!("surface error: {e:?}"),
                    }
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop error");
}

#[allow(clippy::too_many_arguments)]
fn apply_menu_action(
    action: Action,
    mode: &mut Mode,
    session: &mut Option<Session>,
    picker_events: &mut Option<tokio::sync::mpsc::UnboundedReceiver<PickerEvent>>,
    touch: &mut TouchController,
    viewport: (f32, f32),
    elwt: &winit::event_loop::EventLoopWindowTarget<()>,
) {
    match action {
        Action::None => {}
        Action::Quit => elwt.exit(),
        Action::StartSingleplayer => {
            *session = Some(Session::demo());
            *mode = Mode::InGame;
            touch.relayout(viewport);
        }
        Action::GoToMultiplayer => {
            *mode = Mode::Ui(Screen::Multiplayer(ui::screens::MultiplayerScreen::new(viewport, "Cobble")));
        }
        Action::GoToTexturePacks => {
            *mode = Mode::Ui(Screen::TexturePackPicker(TexturePackPickerScreen::new()));
            *picker_events = Some(texturepacks::threaded::search());
        }
        Action::Connect { host, username } => {
            let (host, port) = parse_address(&host);
            log::info!("connecting to {host}:{port} as {username}...");
            *session = Some(Session::networked(protocol::connect(host, port, username)));
            *mode = Mode::InGame;
            touch.relayout(viewport);
        }
        Action::BackToMenu => *mode = Mode::Ui(Screen::MainMenu),
        Action::RequestTexturePackSearch => {
            *picker_events = Some(texturepacks::threaded::search());
        }
        Action::SelectTexturePack { index } => {
            if let Mode::Ui(Screen::TexturePackPicker(picker)) = mode {
                if let PickerStatus::Loaded(hits) = &picker.status {
                    if let Some(hit) = hits.get(index).cloned() {
                        picker.status = PickerStatus::Downloading { title: hit.title.clone() };
                        *picker_events = Some(texturepacks::threaded::download_and_load(hit));
                    }
                }
            }
        }
    }
}

fn poll_picker_events(
    picker_events: &mut Option<tokio::sync::mpsc::UnboundedReceiver<PickerEvent>>,
    screen: &mut Screen,
    gpu: &mut GpuState,
    current_atlas: &mut texturepacks::TextureAtlas,
    session: &mut Option<Session>,
) {
    let Some(rx) = picker_events else { return };
    let Screen::TexturePackPicker(picker) = screen else {
        return;
    };

    match rx.try_recv() {
        Ok(PickerEvent::SearchResults(Ok(hits))) => {
            picker.status = PickerStatus::Loaded(hits);
            *picker_events = None;
        }
        Ok(PickerEvent::SearchResults(Err(message))) => {
            picker.status = PickerStatus::Error(message);
            *picker_events = None;
        }
        Ok(PickerEvent::PackLoaded(Ok((title, loaded)))) => {
            let coverage_percent = loaded.coverage.percentage();
            gpu.set_atlas_texture(&loaded.atlas);
            *current_atlas = loaded.atlas;
            if let Some(active_session) = session {
                active_session.world_dirty = true;
            }
            picker.status = PickerStatus::PackReady { title, coverage_percent };
            *picker_events = None;
        }
        Ok(PickerEvent::PackLoaded(Err(message))) => {
            picker.status = PickerStatus::Error(message);
            *picker_events = None;
        }
        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => *picker_events = None,
    }
}

fn parse_address(addr: &str) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => (host.to_string(), port),
            Err(_) => (addr.to_string(), 25565),
        },
        None => (addr.to_string(), 25565),
    }
}
