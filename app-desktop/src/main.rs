//! Desktop entry point (Windows/Linux).
//!
//! Roadmap step 1: hardcoded demo chunk, no networking. (still
//! available: run with no arguments.)
//!
//! Roadmap step 2 (this file now): `cobble <host[:port]> [username]`
//! connects to a real Minecraft 1.8.9 server over `protocol`, receives
//! real Chunk Data, and renders the actual world instead of the demo
//! chunk. Movement is still a free-fly camera with no physics/collision
//! (that's step 3) — we send position updates to the server so it
//! doesn't think we've disappeared, but nothing stops us from flying
//! through walls yet.

mod network;

use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::{Camera, Chunk, InputState, World};
use glam::Vec3;
use network::OutgoingPosition;
use protocol::GameEvent;
use renderer::{mesh_chunk, mesh_world, GpuState};
use winit::{
    event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, WindowBuilder},
};

const MOVE_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.0025;
const POSITION_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

/// Everything about the current session that differs between the
/// no-network demo chunk and a real server connection.
enum Session {
    Demo,
    Networked {
        net: network::NetworkHandle,
        world: World,
        world_dirty: bool,
        last_position_sent: Instant,
    },
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
    let mut camera = Camera::new(Vec3::new(-6.0, 14.0, -6.0), aspect);

    let mut session = match server_arg {
        Some(addr) => {
            let (host, port) = parse_address(&addr);
            log::info!("connecting to {host}:{port} as {username}...");
            Session::Networked {
                net: network::connect(host, port, username),
                world: World::new(),
                world_dirty: false,
                last_position_sent: Instant::now(),
            }
        }
        None => {
            log::info!("no server given, showing the hardcoded demo chunk (see: cobble <host[:port]> [username])");
            let chunk = Chunk::hardcoded_demo();
            let (vertices, indices) = mesh_chunk(&chunk);
            gpu.set_chunk_mesh(&vertices, &indices);
            Session::Demo
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

                        if let Session::Networked {
                            net,
                            world,
                            world_dirty,
                            ..
                        } = &mut session
                        {
                            drain_network_events(net, world, world_dirty, &mut camera);
                            if *world_dirty {
                                let (vertices, indices) = mesh_world(world);
                                gpu.set_chunk_mesh(&vertices, &indices);
                                *world_dirty = false;
                            }
                        }

                        let (dx, dy) = input.take_look_delta();
                        if mouse_captured {
                            camera.rotate(dx * MOUSE_SENSITIVITY, -dy * MOUSE_SENSITIVITY);
                        }

                        let mut move_dir = Vec3::ZERO;
                        let forward = camera.forward();
                        let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
                        let right = camera.right();
                        if input.move_forward {
                            move_dir += flat_forward;
                        }
                        if input.move_backward {
                            move_dir -= flat_forward;
                        }
                        if input.move_right {
                            move_dir += right;
                        }
                        if input.move_left {
                            move_dir -= right;
                        }
                        if input.jump {
                            move_dir += Vec3::Y;
                        }
                        if input.sneak {
                            move_dir -= Vec3::Y;
                        }
                        if move_dir.length_squared() > 0.0 {
                            camera.position += move_dir.normalize() * MOVE_SPEED * dt;
                        }

                        if let Session::Networked {
                            net,
                            last_position_sent,
                            ..
                        } = &mut session
                        {
                            if now.duration_since(*last_position_sent) >= POSITION_UPDATE_INTERVAL {
                                *last_position_sent = now;
                                let yaw_deg = camera.yaw.to_degrees() + 90.0;
                                let pitch_deg = -camera.pitch.to_degrees();
                                net.send_position(OutgoingPosition {
                                    x: camera.position.x as f64,
                                    y: camera.position.y as f64,
                                    z: camera.position.z as f64,
                                    yaw: yaw_deg,
                                    pitch: pitch_deg,
                                    on_ground: false,
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

/// Applies every network event queued since the last frame to `world`
/// and `camera`, without blocking if none are ready yet.
fn drain_network_events(
    net: &mut network::NetworkHandle,
    world: &mut World,
    world_dirty: &mut bool,
    camera: &mut Camera,
) {
    loop {
        match net.events.try_recv() {
            Ok(GameEvent::JoinGame { dimension, .. }) => {
                log::info!("joined game in dimension {dimension}");
            }
            Ok(GameEvent::ChunkColumnLoaded(column)) => {
                world.insert_column(column);
                *world_dirty = true;
            }
            Ok(GameEvent::ChunkColumnUnloaded { chunk_x, chunk_z }) => {
                world.remove_column(chunk_x, chunk_z);
                *world_dirty = true;
            }
            Ok(GameEvent::PlayerPositionAndLook { x, y, z, yaw, pitch }) => {
                camera.position = Vec3::new(x as f32, y as f32, z as f32);
                camera.yaw = (yaw - 90.0).to_radians();
                camera.pitch = (-pitch).to_radians();
                log::info!("spawned at ({x:.1}, {y:.1}, {z:.1})");
            }
            Ok(GameEvent::BlockChange { x, y, z, block }) => {
                world.set_block(x, y, z, block);
                *world_dirty = true;
            }
            Ok(GameEvent::ChatMessage(json)) => {
                log::info!("chat: {json}");
            }
            Ok(GameEvent::Disconnected(reason)) => {
                log::warn!("disconnected: {reason}");
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}
