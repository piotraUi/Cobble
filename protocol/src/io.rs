//! Typed reader/writer for packet bodies, on top of the raw VarInt
//! helpers in `varint`. Field encodings follow wiki.vg's "Data types"
//! page for the 1.8.9 protocol.

use bytes::{Buf, Bytes};

use crate::error::{ProtocolError, Result};
use crate::varint::{read_varint, read_varlong, write_varint, write_varlong};

pub struct PacketReader {
    buf: Bytes,
}

impl PacketReader {
    pub fn new(buf: Bytes) -> Self {
        Self { buf }
    }

    pub fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    fn require(&self, n: usize) -> Result<()> {
        if self.buf.remaining() < n {
            Err(ProtocolError::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    pub fn read_varint(&mut self) -> Result<i32> {
        read_varint(&mut self.buf)
    }

    pub fn read_varlong(&mut self) -> Result<i64> {
        read_varlong(&mut self.buf)
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.require(1)?;
        Ok(self.buf.get_u8())
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        self.require(1)?;
        Ok(self.buf.get_i8())
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        self.require(2)?;
        Ok(self.buf.get_u16())
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        self.require(2)?;
        Ok(self.buf.get_i16())
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        self.require(4)?;
        Ok(self.buf.get_i32())
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        self.require(8)?;
        Ok(self.buf.get_i64())
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        self.require(4)?;
        Ok(self.buf.get_f32())
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        self.require(8)?;
        Ok(self.buf.get_f64())
    }

    pub fn read_string(&mut self) -> Result<String> {
        let len = self.read_varint()? as usize;
        self.require(len)?;
        let mut bytes = vec![0u8; len];
        self.buf.copy_to_slice(&mut bytes);
        String::from_utf8(bytes).map_err(|_| ProtocolError::InvalidString)
    }

    /// 1.8.9-style dashed-hex UUID string, as used in the Login Success
    /// packet (entity UUIDs elsewhere on the wire are raw 16 bytes; see
    /// `read_uuid_bytes`).
    pub fn read_uuid_string(&mut self) -> Result<String> {
        self.read_string()
    }

    pub fn read_uuid_bytes(&mut self) -> Result<u128> {
        self.require(16)?;
        Ok(self.buf.get_u128())
    }

    /// Consumes and returns exactly `len` raw bytes.
    pub fn read_bytes(&mut self, len: usize) -> Result<Bytes> {
        self.require(len)?;
        Ok(self.buf.copy_to_bytes(len))
    }

    /// Consumes and returns all remaining bytes in the packet.
    pub fn read_remaining(&mut self) -> Bytes {
        self.buf.copy_to_bytes(self.buf.remaining())
    }

    /// Decodes the 1.8.9 block Position type: a single i64 packing
    /// x (26 bits), y (12 bits), z (26 bits).
    pub fn read_position(&mut self) -> Result<(i32, i32, i32)> {
        let val = self.read_i64()?;
        // x and z fall out of the arithmetic (sign-extending) shifts
        // already correctly signed; y needs a manual two's-complement
        // fixup since masking with 0xFFF discards its sign bit.
        let x = (val >> 38) as i32;
        let mut y = ((val >> 26) & 0xFFF) as i32;
        let z = (val << 38 >> 38) as i32;
        if y >= 1 << 11 {
            y -= 1 << 12;
        }
        Ok((x, y, z))
    }
}

#[derive(Default)]
pub struct PacketWriter {
    buf: Vec<u8>,
}

impl PacketWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn write_varint(&mut self, value: i32) -> &mut Self {
        write_varint(&mut self.buf, value);
        self
    }

    pub fn write_varlong(&mut self, value: i64) -> &mut Self {
        write_varlong(&mut self.buf, value);
        self
    }

    pub fn write_bool(&mut self, value: bool) -> &mut Self {
        self.buf.push(value as u8);
        self
    }

    pub fn write_u8(&mut self, value: u8) -> &mut Self {
        self.buf.push(value);
        self
    }

    pub fn write_i16(&mut self, value: i16) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_i32(&mut self, value: i32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_i64(&mut self, value: i64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_f32(&mut self, value: f32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_f64(&mut self, value: f64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_string(&mut self, value: &str) -> &mut Self {
        write_varint(&mut self.buf, value.len() as i32);
        self.buf.extend_from_slice(value.as_bytes());
        self
    }

    pub fn write_bytes(&mut self, value: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(value);
        self
    }

    /// Encodes the 1.8.9 block Position type (see `PacketReader::read_position`).
    pub fn write_position(&mut self, x: i32, y: i32, z: i32) -> &mut Self {
        let val = ((x as i64 & 0x3FFFFFF) << 38)
            | ((y as i64 & 0xFFF) << 26)
            | (z as i64 & 0x3FFFFFF);
        self.write_i64(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_round_trips_including_negatives_and_extremes() {
        let cases = [
            (0, 0, 0),
            (1, 2, 3),
            (-1, -1, -1),
            (18999, 255, -19000),
            (33554431, 2047, -33554432),  // max/min in-range values
            (-33554432, -2048, 33554431),
        ];
        for (x, y, z) in cases {
            let mut w = PacketWriter::new();
            w.write_position(x, y, z);
            let mut r = PacketReader::new(Bytes::from(w.into_bytes()));
            assert_eq!(r.read_position().unwrap(), (x, y, z), "position ({x},{y},{z})");
        }
    }

    #[test]
    fn string_round_trips() {
        let mut w = PacketWriter::new();
        w.write_string("Cobble łąka 😀");
        let mut r = PacketReader::new(Bytes::from(w.into_bytes()));
        assert_eq!(r.read_string().unwrap(), "Cobble łąka 😀");
    }
}
