//! Desktop entry point (Windows/Linux). Roadmap step 1: open a window,
//! render a single hardcoded chunk of colored cubes with a free-fly FPS
//! camera driven by WASD + mouse look. No networking, no texture packs
//! yet — see /protocol and /texturepacks for those.

use std::sync::Arc;
use std::time::Instant;

use client_core::{Camera, Chunk, InputState};
use glam::Vec3;
use renderer::{mesh_chunk, GpuState};
use winit::{
    event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, WindowBuilder},
};

const MOVE_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.0025;

fn main() {
    env_logger::init();

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

    let chunk = Chunk::hardcoded_demo();
    let (vertices, indices) = mesh_chunk(&chunk);
    log::info!(
        "meshed hardcoded demo chunk: {} vertices, {} indices",
        vertices.len(),
        indices.len()
    );
    gpu.set_chunk_mesh(&vertices, &indices);

    let aspect = gpu.size.0 as f32 / gpu.size.1 as f32;
    let mut camera = Camera::new(Vec3::new(-6.0, 14.0, -6.0), aspect);

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
