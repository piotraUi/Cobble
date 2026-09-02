//! Shared game state for Cobble: blocks, chunks, world, camera and the
//! input abstraction used by both the desktop and Android front ends.
//!
//! Networking (protocol crate), rendering (renderer crate) and UI are
//! deliberately kept out of this crate.

pub mod block;
pub mod camera;
pub mod chunk;
pub mod chunk_column;
pub mod input;
pub mod world;

pub use block::BlockId;
pub use camera::Camera;
pub use chunk::{Chunk, CHUNK_SIZE};
pub use chunk_column::{ChunkColumn, SECTIONS_PER_COLUMN, WORLD_HEIGHT};
pub use input::InputState;
pub use world::World;
