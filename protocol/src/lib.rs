//! Minecraft 1.8.9 network protocol (protocol version 47).
//!
//! Covers the Handshaking -> Login -> Play state machine in offline
//! mode (no Mojang auth/encryption), VarInt-prefixed packet framing
//! with optional zlib compression, and the clientbound/serverbound
//! packets needed to join a real 1.8.9 server and see its world: login,
//! chunk data (single and bulk), entity-free player movement, block
//! changes, chat, and keep-alive.
//!
//! Not yet covered: entity spawn/despawn and other entity packets,
//! inventory, and online-mode (Mojang) authentication — see the
//! project roadmap for later steps.

pub mod chunk_data;
pub mod connection;
pub mod error;
pub mod frame;
pub mod io;
pub mod packets;
pub mod state;
pub mod varint;

pub use connection::{GameEvent, MinecraftConnection, PROTOCOL_VERSION};
pub use error::{ProtocolError, Result};
