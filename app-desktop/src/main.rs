//! Desktop entry point (Windows/Linux).
//!
//! Roadmap step 1: hardcoded demo chunk, no networking.
//! Roadmap step 2: `cobble <host[:port]> [username]` connects to a real
//! Minecraft 1.8.9 server and renders the real world.
//! Roadmap step 3: the player is a real AABB with gravity and block
//! collision (see `client_core::physics`) instead of a free-fly
//! camera, in both the demo chunk and networked modes.
//! Roadmap step 5 (this file now): the world renders with a real
//! texture atlas instead of flat debug colors, and there's a
//! Minecraft-styled main menu / texture pack picker / HUD built on the
//! `ui` crate — running `cobble` with no arguments now opens that menu
//! instead of jumping straight to the demo chunk. Passing a server
//! address still skips the menu entirely, for quick testing.

use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::{Camera, Chunk, ChunkColumn, InputState, PlayerPhysics, World};
use glam::Vec3;
use protocol::{GameEvent, OutgoingPosition};
use renderer::{mesh_world, GpuState};
use texturepacks::PickerEvent;
use ui::screens::{draw_hud, Action, PickerStatus, Screen, TexturePackPickerScreen};
use ui::{Font, Painter, UiInput};
use winit::{
    event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, WindowBuilder},
};

const MOUSE_SENSITIVITY: f32 = 0.0025;
const POSITION_UPDATE_INTERVAL: Duration = Duration::from_millis(100);
const FONT_PIXEL_SIZE: f32 = 16.0;

/// Everything about the current game session: the world we're
/// rendering/colliding against, the player's physics state, and
/// (if connected) the network link to a real server.
struct Session {
    world: World,
    physics: PlayerPhysics,
    world_dirty: bool,
    net: Option<protocol::NetworkHandle>,
    /// True once we've received a real spawn position from the server
    /// (or immediately, in demo mode) — until then we don't send
    /// position updates, since we'd just be reporting the placeholder
    /// spawn point.
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

    /// Applies every network event queued since the last frame; a
    /// no-op in demo mode.
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

/// Whether we're showing a menu screen or actually playing.
enum Mode {
    Ui(Screen),
    InGame,
}

fn main() {
    // env_logger prints nothing at all unless RUST_LOG is set, which
    // makes the console window look "empty" even when something (e.g.
    // a server disconnect reason) was worth seeing — default to info
    // so it's actually useful out of the box; RUST_LOG can still raise/
    // lower this.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut args = std::env::args().skip(1);
    let server_arg = args.next();
    let username = args.next().unwrap_or_else(|| "Cobble".to_string());

    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Cobble")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .expect("failed to create window"),
    );

    let mut gpu = pollster::block_on(GpuState::new(window.clone()));
    let mut scale = window.scale_factor() as f32;
    gpu.set_ui_scale(scale);
    let aspect = gpu.size.0 as f32 / gpu.size.1 as f32;
    let mut camera = Camera::new(Vec3::new(8.0, 20.0, 8.0), aspect);

    // Every block always renders with *some* texture, even before the
    // player has picked a pack — see `texturepacks::build_fallback_atlas`.
    let mut current_atlas = texturepacks::build_fallback_atlas();
    gpu.set_atlas_texture(&current_atlas);

    let ui_font = Font::load_regular(FONT_PIXEL_SIZE);
    gpu.set_ui_texture(&ui_font.atlas);

    let mut session: Option<Session> = None;
    let mut mode = match server_arg {
        Some(addr) => {
            let (host, port) = parse_address(&addr);
            log::info!("connecting to {host}:{port} as {username}...");
            session = Some(Session::networked(protocol::connect(host, port, username)));
            Mode::InGame
        }
        None => Mode::Ui(Screen::MainMenu),
    };

    let mut input = InputState::default();
    let mut ui_input = UiInput::default();
    let mut mouse_captured = false;
    let mut last_frame = Instant::now();
    let mut picker_events: Option<tokio::sync::mpsc::UnboundedReceiver<PickerEvent>> = None;

    if matches!(mode, Mode::InGame) {
        capture_mouse(&window, &mut mouse_captured);
    }

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(size) => gpu.resize((size.width, size.height)),
                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        scale = scale_factor as f32;
                        gpu.set_ui_scale(scale);
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        ui_input.mouse_pos = (position.x as f32 / scale, position.y as f32 / scale);
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => match mode {
                        Mode::Ui(_) => ui_input.clicked = true,
                        Mode::InGame => capture_mouse(&window, &mut mouse_captured),
                    },
                    WindowEvent::KeyboardInput { event, .. } => {
                        let pressed = event.state == ElementState::Pressed;
                        match &mode {
                            Mode::Ui(_) => {
                                if pressed {
                                    match event.physical_key {
                                        PhysicalKey::Code(KeyCode::Backspace) => ui_input.backspace = true,
                                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => ui_input.enter = true,
                                        _ => {
                                            if let Some(text) = &event.text {
                                                ui_input.text_input.push_str(text);
                                            }
                                        }
                                    }
                                }
                            }
                            Mode::InGame => match event.physical_key {
                                PhysicalKey::Code(KeyCode::KeyW) => input.move_forward = pressed,
                                PhysicalKey::Code(KeyCode::KeyS) => input.move_backward = pressed,
                                PhysicalKey::Code(KeyCode::KeyA) => input.move_left = pressed,
                                PhysicalKey::Code(KeyCode::KeyD) => input.move_right = pressed,
                                PhysicalKey::Code(KeyCode::Space) => input.jump = pressed,
                                PhysicalKey::Code(KeyCode::ShiftLeft) => input.sneak = pressed,
                                PhysicalKey::Code(KeyCode::Escape) if pressed => {
                                    if mouse_captured {
                                        release_mouse(&window, &mut mouse_captured);
                                    } else {
                                        session = None;
                                        mode = Mode::Ui(Screen::MainMenu);
                                    }
                                }
                                _ => {}
                            },
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32();
                        last_frame = now;
                        let viewport = gpu.ui_viewport();

                        match &mut mode {
                            Mode::Ui(screen) => {
                                poll_picker_events(&mut picker_events, screen, &mut gpu, &mut current_atlas, &mut session);

                                let frame_input = ui_input.take();
                                let action = screen.update(&frame_input, viewport);

                                let mut painter = Painter::new(&ui_font);
                                screen.draw(&mut painter, viewport, frame_input.mouse_pos);
                                gpu.set_ui_draw_list(&painter.list);

                                apply_menu_action(
                                    action,
                                    &mut mode,
                                    &mut session,
                                    &mut picker_events,
                                    &window,
                                    &mut mouse_captured,
                                    viewport,
                                    elwt,
                                );
                            }
                            Mode::InGame => {
                                let Some(active_session) = &mut session else {
                                    // Shouldn't happen, but don't crash if it does.
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

                                let (dx, dy) = input.take_look_delta();
                                if mouse_captured {
                                    camera.rotate(dx * MOUSE_SENSITIVITY, -dy * MOUSE_SENSITIVITY);
                                }

                                if active_session.ready {
                                    let forward = camera.forward();
                                    let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
                                    let right = camera.right();
                                    let mut wish_dir = Vec3::ZERO;
                                    if input.move_forward {
                                        wish_dir += flat_forward;
                                    }
                                    if input.move_backward {
                                        wish_dir -= flat_forward;
                                    }
                                    if input.move_right {
                                        wish_dir += right;
                                    }
                                    if input.move_left {
                                        wish_dir -= right;
                                    }

                                    // dt can spike after e.g. an OS-level stall; clamp it
                                    // so physics never takes one giant, tunneling-prone step.
                                    let physics_dt = dt.min(1.0 / 20.0);
                                    active_session.physics.update(
                                        &active_session.world,
                                        wish_dir,
                                        input.jump,
                                        input.sneak,
                                        physics_dt,
                                    );
                                }
                                camera.position = active_session.physics.eye_position();

                                if let Some(net) = &active_session.net {
                                    if active_session.ready
                                        && now.duration_since(active_session.last_position_sent) >= POSITION_UPDATE_INTERVAL
                                    {
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
                                gpu.set_ui_draw_list(&painter.list);
                            }
                        }

                        match gpu.render() {
                            Ok(()) => {}
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                gpu.resize(gpu.size)
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                            Err(e) => log::warn!("surface error: {e:?}"),
                        }
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if mouse_captured {
                    input.look_delta.0 += delta.0 as f32;
                    input.look_delta.1 += delta.1 as f32;
                }
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop error");
}

fn capture_mouse(window: &winit::window::Window, mouse_captured: &mut bool) {
    *mouse_captured = true;
    let _ = window
        .set_cursor_grab(CursorGrabMode::Locked)
        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
    window.set_cursor_visible(false);
}

fn release_mouse(window: &winit::window::Window, mouse_captured: &mut bool) {
    *mouse_captured = false;
    let _ = window.set_cursor_grab(CursorGrabMode::None);
    window.set_cursor_visible(true);
}

/// Applies whatever the current screen's `update` returned: menu
/// navigation, quitting, or entering the game (single-player or a
/// fresh server connection).
#[allow(clippy::too_many_arguments)]
fn apply_menu_action(
    action: Action,
    mode: &mut Mode,
    session: &mut Option<Session>,
    picker_events: &mut Option<tokio::sync::mpsc::UnboundedReceiver<PickerEvent>>,
    window: &winit::window::Window,
    mouse_captured: &mut bool,
    viewport: (f32, f32),
    elwt: &winit::event_loop::EventLoopWindowTarget<()>,
) {
    match action {
        Action::None => {}
        Action::Quit => elwt.exit(),
        Action::StartSingleplayer => {
            *session = Some(Session::demo());
            *mode = Mode::InGame;
            capture_mouse(window, mouse_captured);
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
            capture_mouse(window, mouse_captured);
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
                        let cache_root = texturepacks::default_cache_dir()
                            .unwrap_or_else(|_| std::env::temp_dir().join("cobble-texturepacks"));
                        *picker_events = Some(texturepacks::threaded::download_and_load(hit, cache_root));
                    }
                }
            }
        }
    }
}

/// Drains any pending texture-pack search/download results and applies
/// them to the active picker screen (and, once a pack finishes
/// loading, the GPU atlas + world mesh).
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
                active_session.world_dirty = true; // force a remesh against the new atlas
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
