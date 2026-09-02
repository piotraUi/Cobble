//! Voxel renderer built on `wgpu`. For roadmap step 1 this only knows
//! how to mesh a `client_core::Chunk` with simple face culling and draw
//! it with flat per-block debug colors (no textures, no texture atlas
//! yet — see roadmap steps 2/4 for network chunks and real textures).

mod gpu;
mod mesher;
mod vertex;

pub use gpu::GpuState;
pub use mesher::{mesh_chunk, mesh_world};
pub use vertex::Vertex;
