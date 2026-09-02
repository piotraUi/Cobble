use crate::error::Result;
use crate::io::{PacketReader, PacketWriter};

pub const LOGIN_DISCONNECT: i32 = 0x00;
/// Sent by online-mode (premium-only) servers to start the Mojang auth
/// handshake — Cobble doesn't implement that yet (see `write_login_start`),
/// so this is only ever used to recognize it and fail with a clear
/// message instead of silently hanging waiting for a `LOGIN_SUCCESS`
/// that will never come.
pub const LOGIN_ENCRYPTION_REQUEST: i32 = 0x01;
pub const LOGIN_SUCCESS: i32 = 0x02;
pub const LOGIN_SET_COMPRESSION: i32 = 0x03;

/// Serverbound Login Start (id 0x00). We only support offline-mode
/// login (no Mojang auth, no encryption), which is what the roadmap
/// asks for at this stage.
pub fn write_login_start(username: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(username);
    w.into_bytes()
}

pub struct LoginSuccess {
    pub uuid: String,
    pub username: String,
}

pub fn parse_login_success(mut r: PacketReader) -> Result<LoginSuccess> {
    Ok(LoginSuccess {
        uuid: r.read_uuid_string()?,
        username: r.read_string()?,
    })
}

pub fn parse_set_compression(mut r: PacketReader) -> Result<i32> {
    r.read_varint()
}

pub fn parse_login_disconnect(mut r: PacketReader) -> Result<String> {
    r.read_string()
}
