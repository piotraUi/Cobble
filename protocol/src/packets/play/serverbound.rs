use crate::io::PacketWriter;

pub const KEEP_ALIVE: i32 = 0x00;
pub const CHAT: i32 = 0x01;
pub const PLAYER: i32 = 0x03;
pub const PLAYER_POSITION: i32 = 0x04;
pub const PLAYER_LOOK: i32 = 0x05;
pub const PLAYER_POSITION_AND_LOOK: i32 = 0x06;

pub fn write_keep_alive(id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_varint(id);
    w.into_bytes()
}

pub fn write_chat(message: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(message);
    w.into_bytes()
}

pub fn write_player(on_ground: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_bool(on_ground);
    w.into_bytes()
}

pub fn write_player_position_and_look(
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_f64(x)
        .write_f64(y)
        .write_f64(z)
        .write_f32(yaw)
        .write_f32(pitch)
        .write_bool(on_ground);
    w.into_bytes()
}
