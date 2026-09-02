//! Desktop entry point (Windows/Linux).
//!
//! Roadmap step 1: hardcoded demo chunk, no networking.
//! Roadmap step 2: `cobble <host[:port]> [username]` connects to a real
//! Minecraft 1.8.9 server and renders the real world.
//! Roadmap step 3: the player is a real AABB with gravity and block
//! collision (see `client_core::physics`) instead of a free-fly
//! camera, in both the demo chunk and networked modes.
//! Roadmap step 5 (this file now, partial): the world renders with a
//! real texture atlas (nearest-neighbor sampled) instead of flat debug
//! colors. No texture pack picker UI yet — that (and the rest of the
//! Minecraft-styled menu/HUD) is still to come, so we always start
//! from the fallback-only atlas (`texturepacks::build_fallback_atlas`).

mod network;

use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::{Camera, Chunk, ChunkColumn, InputState, PlayerPhysics, World};
use glam::Vec3;
use network::OutgoingPosition;
use protocol::GameEvent;
use renderer::{mesh_world, GpuState};
use winit::{
    event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, WindowBuilder},
};

const MOUSE_SENSITIVITY: f32 = 0.0025;
const POSITION_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

/// Everything about the current game session: the world we're
/// rendering/colliding against, the player's physics state, and
/// (if connected) the network link to a real server.
struct Session {
    world: World,
    physics: PlayerPhysics,
    world_dirty: bool,
    net: Option<network::NetworkHandle>,
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

    fn networked(net: network::NetworkHandle) -> Self {
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

fn main() {
    env_logger::init();

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
    let aspect = gpu.size.0 as f32 / gpu.size.1 as f32;
    let mut camera = Camera::new(Vec3::new(8.0, 20.0, 8.0), aspect);

    // No texture pack picker yet (that's the rest of roadmap step 5) —
    // start from the fallback-only atlas so blocks always render with
    // *some* texture instead of flat debug colors.
    let atlas = texturepacks::build_fallback_atlas();
    gpu.set_atlas_texture(&atlas);

    let mut session = match server_arg {
        Some(addr) => {
            let (host, port) = parse_address(&addr);
            log::info!("connecting to {host}:{port} as {username}...");
            Session::networked(network::connect(host, port, username))
        }
        None => {
            log::info!("no server given, showing the hardcoded demo chunk (see: cobble <host[:port]> [username])");
            Session::demo()
        }
    };

    let mut input = InputState::default();
    let mut mouse_captured = false;
    let mut last_frame = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(size) => gpu.resize((size.width, size.height)),
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => {
                        mouse_captured = true;
                        let _ = window
                            .set_cursor_grab(CursorGrabMode::Locked)
                            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
                        window.set_cursor_visible(false);
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        let pressed = event.state == ElementState::Pressed;
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::KeyW) => input.move_forward = pressed,
                            PhysicalKey::Code(KeyCode::KeyS) => input.move_backward = pressed,
                            PhysicalKey::Code(KeyCode::KeyA) => input.move_left = pressed,
                            PhysicalKey::Code(KeyCode::KeyD) => input.move_right = pressed,
                            PhysicalKey::Code(KeyCode::Space) => input.jump = pressed,
                            PhysicalKey::Code(KeyCode::ShiftLeft) => input.sneak = pressed,
                            PhysicalKey::Code(KeyCode::Escape) if pressed => {
                                mouse_captured = false;
                                let _ = window.set_cursor_grab(CursorGrabMode::None);
                                window.set_cursor_visible(true);
                            }
                            _ => {}
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32();
                        last_frame = now;

                        session.drain_network_events();
                        if session.world_dirty {
                            let (vertices, indices) = mesh_world(&session.world, &atlas);
                            if !indices.is_empty() {
                                gpu.set_chunk_mesh(&vertices, &indices);
                            }
                            session.world_dirty = false;
                        }

                        let (dx, dy) = input.take_look_delta();
                        if mouse_captured {
                            camera.rotate(dx * MOUSE_SENSITIVITY, -dy * MOUSE_SENSITIVITY);
                        }

                        if session.ready {
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

                            // dt can spike after e.g. an OS-level stall; clamp it so
                            // physics never takes one giant, tunneling-prone step.
                            let physics_dt = dt.min(1.0 / 20.0);
                            session
                                .physics
                                .update(&session.world, wish_dir, input.jump, input.sneak, physics_dt);
                        }
                        camera.position = session.physics.eye_position();

                        if let Some(net) = &session.net {
                            if session.ready && now.duration_since(session.last_position_sent) >= POSITION_UPDATE_INTERVAL {
                                session.last_position_sent = now;
                                let yaw_deg = camera.yaw.to_degrees() + 90.0;
                                let pitch_deg = -camera.pitch.to_degrees();
                                net.send_position(OutgoingPosition {
                                    x: session.physics.position.x as f64,
                                    y: session.physics.position.y as f64,
                                    z: session.physics.position.z as f64,
                                    yaw: yaw_deg,
                                    pitch: pitch_deg,
                                    on_ground: session.physics.on_ground,
                                });
                            }
                        }

                        gpu.update_camera(camera.view_proj_matrix());
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

fn parse_address(addr: &str) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => (host.to_string(), port),
            Err(_) => (addr.to_string(), 25565),
        },
        None => (addr.to_string(), 25565),
    }
}
