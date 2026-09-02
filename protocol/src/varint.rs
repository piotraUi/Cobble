//! VarInt/VarLong encoding as used throughout the Minecraft protocol
//! (little end first, 7 data bits + continuation bit per byte, LSB
//! first). See <https://wiki.vg/Protocol#VarInt_and_VarLong>.

use bytes::{Buf, BufMut};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{ProtocolError, Result};

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

pub fn write_varint(buf: &mut impl BufMut, mut value: i32) {
    loop {
        let byte = (value as u32 & SEGMENT_BITS as u32) as u8;
        value = ((value as u32) >> 7) as i32;
        if value == 0 {
            buf.put_u8(byte);
            return;
        }
        buf.put_u8(byte | CONTINUE_BIT);
    }
}

pub fn varint_size(value: i32) -> usize {
    let mut size = 0;
    let mut v = value as u32;
    loop {
        size += 1;
        v >>= 7;
        if v == 0 {
            return size;
        }
    }
}

pub fn write_varlong(buf: &mut impl BufMut, mut value: i64) {
    loop {
        let byte = (value as u64 & SEGMENT_BITS as u64) as u8;
        value = ((value as u64) >> 7) as i64;
        if value == 0 {
            buf.put_u8(byte);
            return;
        }
        buf.put_u8(byte | CONTINUE_BIT);
    }
}

/// Reads a VarInt from an in-memory buffer (already-received packet body).
pub fn read_varint(buf: &mut impl Buf) -> Result<i32> {
    let mut value: i32 = 0;
    let mut position = 0u32;
    loop {
        if !buf.has_remaining() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let byte = buf.get_u8();
        value |= ((byte & SEGMENT_BITS) as i32) << position;
        if byte & CONTINUE_BIT == 0 {
            return Ok(value);
        }
        position += 7;
        if position >= 32 {
            return Err(ProtocolError::VarIntTooLarge);
        }
    }
}

pub fn read_varlong(buf: &mut impl Buf) -> Result<i64> {
    let mut value: i64 = 0;
    let mut position = 0u32;
    loop {
        if !buf.has_remaining() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let byte = buf.get_u8();
        value |= ((byte & SEGMENT_BITS) as i64) << position;
        if byte & CONTINUE_BIT == 0 {
            return Ok(value);
        }
        position += 7;
        if position >= 64 {
            return Err(ProtocolError::VarLongTooLarge);
        }
    }
}

/// Reads a VarInt directly off an async stream, one byte at a time —
/// used only for the outermost frame length prefix, before we know how
/// many bytes the rest of the packet occupies.
pub async fn read_varint_async<R: AsyncRead + Unpin>(reader: &mut R) -> Result<i32> {
    let mut value: i32 = 0;
    let mut position = 0u32;
    loop {
        let byte = reader.read_u8().await?;
        value |= ((byte & SEGMENT_BITS) as i32) << position;
        if byte & CONTINUE_BIT == 0 {
            return Ok(value);
        }
        position += 7;
        if position >= 32 {
            return Err(ProtocolError::VarIntTooLarge);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn varint_round_trips_known_values() {
        // Reference values from wiki.vg's VarInt examples.
        let cases: &[(i32, &[u8])] = &[
            (0, &[0x00]),
            (1, &[0x01]),
            (2, &[0x02]),
            (127, &[0x7f]),
            (128, &[0x80, 0x01]),
            (255, &[0xff, 0x01]),
            (25565, &[0xdd, 0xc7, 0x01]),
            (2147483647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
            (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
            (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0x08]),
        ];

        for (value, encoded) in cases {
            let mut buf = BytesMut::new();
            write_varint(&mut buf, *value);
            assert_eq!(&buf[..], *encoded, "encoding {value}");

            let mut cursor = &buf[..];
            assert_eq!(read_varint(&mut cursor).unwrap(), *value, "decoding {value}");
        }
    }

    #[test]
    fn varint_roundtrip_fuzz() {
        for value in [i32::MIN, i32::MAX, 0, -1, 1, 42, -42, 300_000, -300_000] {
            let mut buf = BytesMut::new();
            write_varint(&mut buf, value);
            let mut cursor = &buf[..];
            assert_eq!(read_varint(&mut cursor).unwrap(), value);
        }
    }
}
