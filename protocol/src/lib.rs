//! Minecraft 1.8.9 network protocol (protocol version 47).
//!
//! Not implemented yet — this is roadmap step 2. It will cover the
//! Handshaking -> Status/Login -> Play state machine, VarInt-prefixed
//! packet (de)serialization, zlib packet compression, and the
//! clientbound/serverbound packet types needed to join a real 1.8.9
//! server (chunk data, entity spawn/despawn, player position/look,
//! block changes, chat).
