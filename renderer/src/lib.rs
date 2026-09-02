//! Voxel renderer built on `wgpu`: meshes a `client_core::World` (or a
//! single `Chunk`) with simple face culling, texture-mapped against a
//! `texturepacks::TextureAtlas` (nearest-neighbor sampled, no texture
//! pack selected yet still renders — see `block_textures` — via a
//! fallback-only atlas the app builds at startup).

mod block_textures;
mod gpu;
mod mesher;
mod vertex;

pub use block_textures::{face_uv, BlockFaces, FaceKind};
pub use gpu::GpuState;
pub use mesher::{mesh_chunk, mesh_world};
pub use vertex::Vertex;
