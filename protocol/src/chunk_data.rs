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

/// Reads one 4-bit value out of a "nibble array" (2 values packed per
/// byte, low nibble first) — the format both light arrays use on the
/// wire. `index` uses the same Y-Z-X ordering as the block array.
fn nibble_at(data: &[u8], index: usize) -> u8 {
    let byte = data[index / 2];
    if index.is_multiple_of(2) {
        byte & 0x0F
    } else {
        byte >> 4
    }
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

    let block_light_bytes = cursor.take(LIGHT_BYTES)?;
    for y in 0..16usize {
        for z in 0..16usize {
            for x in 0..16usize {
                let index = (y * 16 + z) * 16 + x;
                chunk.set_block_light(x, y, z, nibble_at(block_light_bytes, index));
            }
        }
    }

    if has_sky_light {
        let sky_light_bytes = cursor.take(LIGHT_BYTES)?;
        for y in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let index = (y * 16 + z) * 16 + x;
                    chunk.set_sky_light(x, y, z, nibble_at(sky_light_bytes, index));
                }
            }
        }
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

    /// Packs 4096 nibble values (Y-Z-X order, 0-15 each) 2-per-byte,
    /// low nibble first — the inverse of `nibble_at`.
    fn encode_nibbles(value_at: impl Fn(usize, usize, usize) -> u8) -> Vec<u8> {
        let mut bytes = vec![0u8; LIGHT_BYTES];
        for y in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let index = (y * 16 + z) * 16 + x;
                    let value = value_at(x, y, z) & 0x0F;
                    if index % 2 == 0 {
                        bytes[index / 2] |= value;
                    } else {
                        bytes[index / 2] |= value << 4;
                    }
                }
            }
        }
        bytes
    }

    /// Builds one synthetic section (block array + light arrays) the
    /// way a server would put it on the wire: block ids/metadata as
    /// little-endian u16s in Y-Z-X order, then packed light nibbles
    /// (both zeroed by default; use `encode_section_with_light` for a
    /// section with real light values).
    fn encode_section(block_at: impl Fn(usize, usize, usize) -> (u16, u8), has_sky_light: bool) -> Vec<u8> {
        encode_section_with_light(block_at, |_, _, _| 0, |_, _, _| 0, has_sky_light)
    }

    fn encode_section_with_light(
        block_at: impl Fn(usize, usize, usize) -> (u16, u8),
        block_light_at: impl Fn(usize, usize, usize) -> u8,
        sky_light_at: impl Fn(usize, usize, usize) -> u8,
        has_sky_light: bool,
    ) -> Vec<u8> {
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
        out.extend(encode_nibbles(block_light_at));
        if has_sky_light {
            out.extend(encode_nibbles(sky_light_at));
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

    #[test]
    fn nibble_packing_round_trips_low_and_high_nibbles() {
        // index 0 -> byte 0 low nibble, index 1 -> byte 0 high nibble.
        let packed = encode_nibbles(|x, y, z| {
            let index = (y * 16 + z) * 16 + x;
            (index % 16) as u8
        });
        assert_eq!(nibble_at(&packed, 0), 0);
        assert_eq!(nibble_at(&packed, 1), 1);
        assert_eq!(nibble_at(&packed, 14), 14);
        assert_eq!(nibble_at(&packed, 15), 15);
    }

    #[test]
    fn parses_distinct_block_light_and_sky_light_per_block() {
        let section_bytes = encode_section_with_light(
            |_, _, _| (1, 0),
            |x, y, z| ((x + y + z) % 16) as u8, // varies per block, exercises both nibble halves
            |_, _, _| 15,                       // full daylight everywhere
            true,
        );

        let (column, consumed) = parse_column_sections(0, 0, 0b1, true, false, &section_bytes).unwrap();
        assert_eq!(consumed, section_bytes.len());

        assert_eq!(column.get_light(0, 0, 0), (0, 15));
        assert_eq!(column.get_light(5, 3, 2), (10, 15));
        assert_eq!(column.get_light(15, 15, 15), ((15 + 15 + 15) % 16, 15));
    }

    #[test]
    fn no_sky_light_dimension_leaves_sky_light_at_zero() {
        let section_bytes = encode_section_with_light(|_, _, _| (1, 0), |_, _, _| 7, |_, _, _| 15, false);
        let (column, _) = parse_column_sections(0, 0, 0b1, false, false, &section_bytes).unwrap();
        // No sky light bytes were sent (has_sky_light=false), so it
        // should stay at the chunk's zeroed default, not the 15 the
        // encoder would have written had it been included.
        assert_eq!(column.get_light(0, 0, 0), (7, 0));
    }
}
