//! Shared game state for Cobble: blocks, chunks, camera and the
//! input abstraction used by both the desktop and Android front ends.
//!
//! Networking (protocol crate), rendering (renderer crate) and UI are
//! deliberately kept out of this crate.

pub mod block;
pub mod camera;
pub mod chunk;
pub mod input;

pub use block::BlockId;
pub use camera::Camera;
pub use chunk::{Chunk, CHUNK_SIZE};
pub use input::InputState;
