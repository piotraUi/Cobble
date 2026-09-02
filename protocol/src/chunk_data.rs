//! Decodes the raw byte payload of Chunk Data / Chunk Data Bulk
//! packets (protocol 47 / 1.8.x chunk section format: a 16-bit
//! `(block_id << 4) | metadata` value per block, then a block light
//! nibble array, then an optional sky light nibble array, per
//! present section) into `client_core` chunk columns.
//!
//! See <https://wiki.vg/index.php?title=Chunk_Format> (1.8 revision)
//! and PrismarineJS/prismarine-chunk's `src/pc/1.8/section.js` for the
//! exact byte layout this mirrors.

use client_core::chunk::Chunk;
use client_core::chunk_column::ChunkColumn;
use client_core::BlockId;

use crate::error::{ProtocolError, Result};

const BLOCKS_BYTES: usize = 16 * 16 * 16 * 2;
const LIGHT_BYTES: usize = 16 * 16 * 16 / 2;
const BIOME_BYTES: usize = 256;

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.pos + len > self.data.len() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }
}

/// Converts the wire's `(block_id << 4) | metadata` value into the
/// `BlockId` client-core uses for rendering (metadata is dropped until
/// block shapes/variants matter to the renderer).
pub fn block_id_from_raw_state(raw_state: u16) -> BlockId {
    BlockId(raw_state >> 4)
}

fn read_section(cursor: &mut Cursor, has_sky_light: bool) -> Result<Chunk> {
    let block_bytes = cursor.take(BLOCKS_BYTES)?;
    let mut chunk = Chunk::empty();
    for y in 0..16usize {
        for z in 0..16usize {
            for x in 0..16usize {
                let index = (y * 16 + z) * 16 + x;
                let raw = u16::from_le_bytes([block_bytes[index * 2], block_bytes[index * 2 + 1]]);
                chunk.set(x, y, z, block_id_from_raw_state(raw));
            }
        }
    }

    // Block light and (optionally) sky light follow but aren't needed
    // for rendering yet — skip them for now (see roadmap step 7).
    cursor.take(LIGHT_BYTES)?;
    if has_sky_light {
        cursor.take(LIGHT_BYTES)?;
    }

    Ok(chunk)
}

/// Parses a single column's worth of section data (the `chunkData`
/// buffer from a Chunk Data packet, or one column's slice of a Chunk
/// Data Bulk packet's `data` buffer).
pub fn parse_column_sections(
    chunk_x: i32,
    chunk_z: i32,
    bitmask: u16,
    has_sky_light: bool,
    ground_up_continuous: bool,
    data: &[u8],
) -> Result<(ChunkColumn, usize)> {
    let mut cursor = Cursor { data, pos: 0 };
    let mut column = ChunkColumn::empty(chunk_x, chunk_z);

    for section_y in 0..16usize {
        if bitmask & (1 << section_y) != 0 {
            let section = read_section(&mut cursor, has_sky_light)?;
            column.set_section(section_y, section);
        }
    }

    if ground_up_continuous {
        // Biomes: one byte per (x, z) column; not modeled yet, just
        // consumed so a following column in a bulk packet lines up.
        cursor.take(BIOME_BYTES)?;
    }

    Ok((column, cursor.pos))
}

pub struct BulkColumnMeta {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub bitmask: u16,
}

/// Parses a Chunk Data Bulk packet's `data` buffer into one
/// `ChunkColumn` per entry in `metas`, in order. Bulk chunk data is
/// always "ground-up continuous" (full columns, biomes included).
pub fn parse_bulk(
    metas: &[BulkColumnMeta],
    sky_light_sent: bool,
    data: &[u8],
) -> Result<Vec<ChunkColumn>> {
    let mut offset = 0usize;
    let mut columns = Vec::with_capacity(metas.len());
    for meta in metas {
        let (column, consumed) = parse_column_sections(
            meta.chunk_x,
            meta.chunk_z,
            meta.bitmask,
            sky_light_sent,
            true,
            &data[offset..],
        )?;
        offset += consumed;
        columns.push(column);
    }
    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds one synthetic section (block array + light arrays) the
    /// way a server would put it on the wire: block ids/metadata as
    /// little-endian u16s in Y-Z-X order, then zeroed light arrays.
    fn encode_section(block_at: impl Fn(usize, usize, usize) -> (u16, u8), has_sky_light: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(BLOCKS_BYTES + LIGHT_BYTES * 2);
        for y in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let (id, meta) = block_at(x, y, z);
                    let raw = (id << 4) | (meta as u16 & 0xF);
                    out.extend_from_slice(&raw.to_le_bytes());
                }
            }
        }
        out.extend(std::iter::repeat_n(0u8, LIGHT_BYTES));
        if has_sky_light {
            out.extend(std::iter::repeat_n(0u8, LIGHT_BYTES));
        }
        out
    }

    #[test]
    fn parses_single_section_with_known_blocks() {
        let section_bytes = encode_section(
            |x, y, z| {
                if y == 0 {
                    (1, 0) // stone
                } else if x == 5 && y == 5 && z == 5 {
                    (2, 3) // grass with an arbitrary metadata value
                } else {
                    (0, 0) // air
                }
            },
            true,
        );

        let (column, consumed) =
            parse_column_sections(3, -2, 0b1, true, false, &section_bytes).unwrap();

        assert_eq!(consumed, section_bytes.len());
        assert_eq!(column.chunk_x, 3);
        assert_eq!(column.chunk_z, -2);

        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(column.get_block(x, 0, z), BlockId(1));
            }
        }
        assert_eq!(column.get_block(5, 5, 5), BlockId(2));
        assert_eq!(column.get_block(5, 6, 5), BlockId::AIR);
        // A section above the ones present in the bitmask must read as air.
        assert_eq!(column.get_block(0, 20, 0), BlockId::AIR);
    }

    #[test]
    fn skips_absent_sections_via_bitmask() {
        // Only section index 2 is present; the buffer only contains that
        // one section's bytes, and no sky light this time.
        let section_bytes = encode_section(|_, _, _| (7, 0), false); // bedrock everywhere
        let (column, consumed) =
            parse_column_sections(0, 0, 0b100, false, false, &section_bytes).unwrap();

        assert_eq!(consumed, section_bytes.len());
        assert_eq!(column.get_block(0, 0, 0), BlockId::AIR); // section 0 absent
        assert_eq!(column.get_block(0, 32, 0), BlockId(7)); // section 2 (y=32..47) present
    }

    #[test]
    fn ground_up_continuous_consumes_biome_array() {
        let mut data = encode_section(|_, _, _| (1, 0), true);
        data.extend(std::iter::repeat_n(0u8, BIOME_BYTES));

        let (_column, consumed) = parse_column_sections(0, 0, 0b1, true, true, &data).unwrap();
        assert_eq!(consumed, data.len());
    }

    #[test]
    fn truncated_buffer_is_an_error() {
        let mut section_bytes = encode_section(|_, _, _| (1, 0), true);
        section_bytes.truncate(section_bytes.len() - 1);
        assert!(parse_column_sections(0, 0, 0b1, true, false, &section_bytes).is_err());
    }
}
