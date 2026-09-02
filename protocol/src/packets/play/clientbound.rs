//! Clientbound Play-state packet ids (protocol 47 / 1.8.x) and parsing
//! for the subset Cobble currently understands. Everything else is
//! left as raw bytes by the caller and simply dropped.

use crate::error::Result;
use crate::io::PacketReader;

pub const KEEP_ALIVE: i32 = 0x00;
pub const JOIN_GAME: i32 = 0x01;
pub const CHAT_MESSAGE: i32 = 0x02;
pub const SPAWN_POSITION: i32 = 0x05;
pub const PLAYER_POSITION_AND_LOOK: i32 = 0x08;
pub const MAP_CHUNK: i32 = 0x21;
pub const MULTI_BLOCK_CHANGE: i32 = 0x22;
pub const BLOCK_CHANGE: i32 = 0x23;
pub const MAP_CHUNK_BULK: i32 = 0x26;
pub const KICK_DISCONNECT: i32 = 0x40;

pub struct JoinGame {
    pub entity_id: i32,
    pub game_mode: u8,
    pub dimension: i8,
    pub difficulty: u8,
    pub max_players: u8,
    pub level_type: String,
    pub reduced_debug_info: bool,
}

pub fn parse_join_game(mut r: PacketReader) -> Result<JoinGame> {
    Ok(JoinGame {
        entity_id: r.read_i32()?,
        game_mode: r.read_u8()?,
        dimension: r.read_i8()?,
        difficulty: r.read_u8()?,
        max_players: r.read_u8()?,
        level_type: r.read_string()?,
        reduced_debug_info: r.read_bool()?,
    })
}

pub struct PlayerPositionAndLook {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: i8,
}

pub fn parse_player_position_and_look(mut r: PacketReader) -> Result<PlayerPositionAndLook> {
    Ok(PlayerPositionAndLook {
        x: r.read_f64()?,
        y: r.read_f64()?,
        z: r.read_f64()?,
        yaw: r.read_f32()?,
        pitch: r.read_f32()?,
        flags: r.read_i8()?,
    })
}

pub fn parse_keep_alive(mut r: PacketReader) -> Result<i32> {
    r.read_varint()
}

pub fn parse_chat_message(mut r: PacketReader) -> Result<(String, i8)> {
    Ok((r.read_string()?, r.read_i8()?))
}

pub fn parse_spawn_position(mut r: PacketReader) -> Result<(i32, i32, i32)> {
    r.read_position()
}

pub fn parse_kick_disconnect(mut r: PacketReader) -> Result<String> {
    r.read_string()
}

/// A single block change: absolute position plus the packed
/// `(block_id << 4) | metadata` state id, same encoding used inside
/// Chunk Data.
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub raw_state: i32,
}

pub fn parse_block_change(mut r: PacketReader) -> Result<BlockChange> {
    let (x, y, z) = r.read_position()?;
    let raw_state = r.read_varint()?;
    Ok(BlockChange { x, y, z, raw_state })
}

pub struct MultiBlockChangeRecord {
    pub local_x: u8,
    pub y: u8,
    pub local_z: u8,
    pub raw_state: i32,
}

pub struct MultiBlockChange {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub records: Vec<MultiBlockChangeRecord>,
}

pub fn parse_multi_block_change(mut r: PacketReader) -> Result<MultiBlockChange> {
    let chunk_x = r.read_i32()?;
    let chunk_z = r.read_i32()?;
    let count = r.read_varint()? as usize;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let horizontal_pos = r.read_u8()?;
        let y = r.read_u8()?;
        let raw_state = r.read_varint()?;
        records.push(MultiBlockChangeRecord {
            local_x: horizontal_pos >> 4,
            local_z: horizontal_pos & 0x0F,
            y,
            raw_state,
        });
    }
    Ok(MultiBlockChange {
        chunk_x,
        chunk_z,
        records,
    })
}
