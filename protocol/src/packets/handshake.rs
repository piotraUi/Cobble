use crate::io::PacketWriter;

pub const NEXT_STATE_STATUS: i32 = 1;
pub const NEXT_STATE_LOGIN: i32 = 2;

/// Serverbound Handshake (id 0x00, only valid packet in the
/// Handshaking state). Selects whether the connection continues into
/// Status or Login.
pub fn write_handshake(protocol_version: i32, server_address: &str, server_port: u16, next_state: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_varint(protocol_version)
        .write_string(server_address)
        .write_i16(server_port as i16)
        .write_varint(next_state);
    w.into_bytes()
}
