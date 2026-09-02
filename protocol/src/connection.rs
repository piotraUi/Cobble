//! High-level 1.8.9 client connection: handshake + offline-mode login,
//! then a `next_event` loop that decodes the Play-state packets Cobble
//! currently understands (chunks, movement, chat, block changes,
//! disconnects) and transparently answers Keep Alive so the server
//! doesn't time us out.

use std::collections::VecDeque;

use bytes::Bytes;
use client_core::chunk_column::ChunkColumn;
use client_core::BlockId;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use crate::chunk_data::{self, BulkColumnMeta};
use crate::error::{ProtocolError, Result};
use crate::frame::{read_packet, write_packet};
use crate::io::PacketReader;
use crate::packets::{handshake, login, play};

/// Protocol version 47 = Minecraft 1.8 through 1.8.9.
pub const PROTOCOL_VERSION: i32 = 47;

pub struct MinecraftConnection {
    read_half: OwnedReadHalf,
    write_half: OwnedWriteHalf,
    compression_threshold: Option<i32>,
    /// Whether the current dimension has sky light data in its chunk
    /// sections (overworld does, nether/end don't).
    has_sky_light: bool,
    /// Extra events already decoded from a packet that produced more
    /// than one (e.g. Chunk Data Bulk covers many columns at once),
    /// drained before reading the next packet off the wire.
    pending: VecDeque<GameEvent>,
}

pub enum GameEvent {
    JoinGame {
        entity_id: i32,
        dimension: i32,
    },
    ChunkColumnLoaded(ChunkColumn),
    ChunkColumnUnloaded {
        chunk_x: i32,
        chunk_z: i32,
    },
    PlayerPositionAndLook {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
    },
    ChatMessage(String),
    BlockChange {
        x: i32,
        y: i32,
        z: i32,
        block: BlockId,
    },
    Disconnected(String),
}

impl std::fmt::Debug for GameEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JoinGame { entity_id, dimension } => f
                .debug_struct("JoinGame")
                .field("entity_id", entity_id)
                .field("dimension", dimension)
                .finish(),
            Self::ChunkColumnLoaded(column) => f
                .debug_struct("ChunkColumnLoaded")
                .field("chunk_x", &column.chunk_x)
                .field("chunk_z", &column.chunk_z)
                .finish(),
            Self::ChunkColumnUnloaded { chunk_x, chunk_z } => f
                .debug_struct("ChunkColumnUnloaded")
                .field("chunk_x", chunk_x)
                .field("chunk_z", chunk_z)
                .finish(),
            Self::PlayerPositionAndLook { x, y, z, yaw, pitch } => f
                .debug_struct("PlayerPositionAndLook")
                .field("x", x)
                .field("y", y)
                .field("z", z)
                .field("yaw", yaw)
                .field("pitch", pitch)
                .finish(),
            Self::ChatMessage(json) => f.debug_tuple("ChatMessage").field(json).finish(),
            Self::BlockChange { x, y, z, block } => f
                .debug_struct("BlockChange")
                .field("x", x)
                .field("y", y)
                .field("z", z)
                .field("block", block)
                .finish(),
            Self::Disconnected(reason) => f.debug_tuple("Disconnected").field(reason).finish(),
        }
    }
}

impl MinecraftConnection {
    /// Connects to `host:port`, performs the Handshaking -> Login
    /// exchange in offline mode (no Mojang auth/encryption), and
    /// returns a connection ready to receive Play-state events.
    pub async fn connect(host: &str, port: u16, username: &str) -> Result<Self> {
        let stream = TcpStream::connect((host, port)).await?;
        stream.set_nodelay(true).ok();
        let (read_half, mut write_half) = stream.into_split();

        write_packet(
            &mut write_half,
            0x00,
            &handshake::write_handshake(PROTOCOL_VERSION, host, port, handshake::NEXT_STATE_LOGIN),
            None,
        )
        .await?;
        write_packet(&mut write_half, 0x00, &login::write_login_start(username), None).await?;

        let mut conn = Self {
            read_half,
            write_half,
            compression_threshold: None,
            has_sky_light: true,
            pending: VecDeque::new(),
        };

        loop {
            let (packet_id, body) = read_packet(&mut conn.read_half, conn.compression_threshold).await?;
            let r = PacketReader::new(body);
            match packet_id {
                login::LOGIN_SET_COMPRESSION => {
                    let threshold = login::parse_set_compression(r)?;
                    conn.compression_threshold = if threshold >= 0 { Some(threshold) } else { None };
                }
                login::LOGIN_SUCCESS => {
                    let success = login::parse_login_success(r)?;
                    log::info!("logged in as {} ({})", success.username, success.uuid);
                    break;
                }
                login::LOGIN_DISCONNECT => {
                    let reason = login::parse_login_disconnect(r)?;
                    return Err(ProtocolError::Disconnected(reason));
                }
                other => {
                    log::debug!("ignoring unexpected login-state packet 0x{other:02x}");
                }
            }
        }

        Ok(conn)
    }

    /// Reads and decodes the next Play-state packet we understand,
    /// looping past ones we don't and auto-replying to Keep Alive.
    pub async fn next_event(&mut self) -> Result<GameEvent> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(event);
        }
        loop {
            let (packet_id, body) = read_packet(&mut self.read_half, self.compression_threshold).await?;
            let r = PacketReader::new(body);

            match packet_id {
                play::clientbound::KEEP_ALIVE => {
                    let id = play::clientbound::parse_keep_alive(r)?;
                    self.send_raw(play::serverbound::KEEP_ALIVE, &play::serverbound::write_keep_alive(id))
                        .await?;
                }
                play::clientbound::JOIN_GAME => {
                    let join = play::clientbound::parse_join_game(r)?;
                    self.has_sky_light = join.dimension == 0;
                    return Ok(GameEvent::JoinGame {
                        entity_id: join.entity_id,
                        dimension: join.dimension as i32,
                    });
                }
                play::clientbound::CHAT_MESSAGE => {
                    let (json, _position) = play::clientbound::parse_chat_message(r)?;
                    return Ok(GameEvent::ChatMessage(json));
                }
                play::clientbound::PLAYER_POSITION_AND_LOOK => {
                    let pos = play::clientbound::parse_player_position_and_look(r)?;
                    // Confirm the teleport by echoing the same position back,
                    // as required by the protocol (flags/relative handling
                    // is not implemented yet — servers we target send
                    // absolute coordinates for the initial spawn teleport).
                    self.send_player_position_and_look(pos.x, pos.y, pos.z, pos.yaw, pos.pitch, true)
                        .await?;
                    return Ok(GameEvent::PlayerPositionAndLook {
                        x: pos.x,
                        y: pos.y,
                        z: pos.z,
                        yaw: pos.yaw,
                        pitch: pos.pitch,
                    });
                }
                play::clientbound::MAP_CHUNK => {
                    if let Some(event) = self.parse_map_chunk(r)? {
                        return Ok(event);
                    }
                }
                play::clientbound::MAP_CHUNK_BULK => {
                    if let Some(event) = self.parse_map_chunk_bulk(r)? {
                        return Ok(event);
                    }
                }
                play::clientbound::BLOCK_CHANGE => {
                    let change = play::clientbound::parse_block_change(r)?;
                    return Ok(GameEvent::BlockChange {
                        x: change.x,
                        y: change.y,
                        z: change.z,
                        block: BlockId((change.raw_state as u32 >> 4) as u16),
                    });
                }
                play::clientbound::KICK_DISCONNECT => {
                    let reason = play::clientbound::parse_kick_disconnect(r)?;
                    return Ok(GameEvent::Disconnected(reason));
                }
                _ => {
                    // Not modeled yet (entities, inventory, scoreboard, ...) —
                    // safe to drop since we already consumed the whole frame.
                }
            }
        }
    }

    fn parse_map_chunk(&self, mut r: PacketReader) -> Result<Option<GameEvent>> {
        let chunk_x = r.read_i32()?;
        let chunk_z = r.read_i32()?;
        let ground_up_continuous = r.read_bool()?;
        let bitmask = r.read_u16()?;
        let data_len = r.read_varint()? as usize;
        let data = r.read_bytes(data_len)?;

        if ground_up_continuous && bitmask == 0 {
            return Ok(Some(GameEvent::ChunkColumnUnloaded { chunk_x, chunk_z }));
        }

        let (column, _) = chunk_data::parse_column_sections(
            chunk_x,
            chunk_z,
            bitmask,
            self.has_sky_light,
            ground_up_continuous,
            &data,
        )?;
        Ok(Some(GameEvent::ChunkColumnLoaded(column)))
    }

    /// Parses every column in a Chunk Data Bulk packet, returns the
    /// first as this call's event and queues the rest in `pending` so
    /// none are lost (the wire packet carries many columns, but
    /// `next_event` only returns one event at a time).
    fn parse_map_chunk_bulk(&mut self, mut r: PacketReader) -> Result<Option<GameEvent>> {
        let sky_light_sent = r.read_bool()?;
        let count = r.read_varint()? as usize;
        let mut metas = Vec::with_capacity(count);
        for _ in 0..count {
            metas.push(BulkColumnMeta {
                chunk_x: r.read_i32()?,
                chunk_z: r.read_i32()?,
                bitmask: r.read_u16()?,
            });
        }
        let data: Bytes = r.read_remaining();
        let columns = chunk_data::parse_bulk(&metas, sky_light_sent, &data)?;

        let mut columns = columns.into_iter();
        let first = columns.next();
        self.pending
            .extend(columns.map(GameEvent::ChunkColumnLoaded));
        Ok(first.map(GameEvent::ChunkColumnLoaded))
    }

    async fn send_raw(&mut self, packet_id: i32, body: &[u8]) -> Result<()> {
        write_packet(&mut self.write_half, packet_id, body, self.compression_threshold).await
    }

    pub async fn send_chat(&mut self, message: &str) -> Result<()> {
        self.send_raw(play::serverbound::CHAT, &play::serverbound::write_chat(message))
            .await
    }

    pub async fn send_player_position_and_look(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) -> Result<()> {
        self.send_raw(
            play::serverbound::PLAYER_POSITION_AND_LOOK,
            &play::serverbound::write_player_position_and_look(x, y, z, yaw, pitch, on_ground),
        )
        .await
    }
}
