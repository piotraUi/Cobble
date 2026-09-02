#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("varint is too large (more than 5 bytes)")]
    VarIntTooLarge,
    #[error("varlong is too large (more than 10 bytes)")]
    VarLongTooLarge,
    #[error("unexpected end of packet buffer")]
    UnexpectedEof,
    #[error("invalid utf-8 string in packet")]
    InvalidString,
    #[error("packet too large: {0} bytes")]
    PacketTooLarge(usize),
    #[error("unknown packet id 0x{0:02x} in state {1:?}")]
    UnknownPacket(i32, &'static str),
    #[error("server rejected login/status: {0}")]
    Disconnected(String),
    #[error("connection closed by server")]
    ConnectionClosed,
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
