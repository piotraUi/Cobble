use crate::block::BlockId;

pub const CHUNK_SIZE: usize = 16;
const VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

/// Light levels are 0-15, same range as the wire's 4-bit nibble arrays
/// (see `protocol::chunk_data`) — stored unpacked (one byte per block)
/// here since the packing only matters on the wire.
pub const MAX_LIGHT: u8 = 15;

/// A single 16x16x16 chunk section, matching the Minecraft 1.8.9 chunk
/// section size used on the wire (see `protocol`'s Chunk Data packet
/// handling).
pub struct Chunk {
    blocks: [BlockId; VOLUME],
    block_light: [u8; VOLUME],
    sky_light: [u8; VOLUME],
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: [BlockId::AIR; VOLUME],
            block_light: [0; VOLUME],
            sky_light: [0; VOLUME],
        }
    }

    #[inline]
    fn index(x: usize, y: usize, z: usize) -> usize {
        (y * CHUNK_SIZE + z) * CHUNK_SIZE + x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockId {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
            return BlockId::AIR;
        }
        self.blocks[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockId) {
        self.blocks[Self::index(x, y, z)] = block;
    }

    #[inline]
    pub fn block_light(&self, x: usize, y: usize, z: usize) -> u8 {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
            return 0;
        }
        self.block_light[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, level: u8) {
        self.block_light[Self::index(x, y, z)] = level.min(MAX_LIGHT);
    }

    #[inline]
    pub fn sky_light(&self, x: usize, y: usize, z: usize) -> u8 {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
            return MAX_LIGHT;
        }
        self.sky_light[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set_sky_light(&mut self, x: usize, y: usize, z: usize, level: u8) {
        self.sky_light[Self::index(x, y, z)] = level.min(MAX_LIGHT);
    }

    /// Builds a small hardcoded demo terrain: a few stone/dirt/grass
    /// layers with a sand patch and a single tree, used before real
    /// network chunk data is available (see roadmap step 2). Lit as a
    /// plain outdoor scene at full daylight — there's no light
    /// propagation engine here, just the wire format support (see
    /// roadmap step 7); block light stays 0 since nothing here emits any.
    pub fn hardcoded_demo() -> Self {
        let mut chunk = Self::empty();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    chunk.set_sky_light(x, y, z, MAX_LIGHT);
                }
            }
        }

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                chunk.set(x, 0, z, BlockId::BEDROCK);
                for y in 1..4 {
                    chunk.set(x, y, z, BlockId::STONE);
                }
                for y in 4..7 {
                    chunk.set(x, y, z, BlockId::DIRT);
                }
                chunk.set(x, 7, z, BlockId::GRASS);
            }
        }

        // A sand patch in one corner.
        for x in 0..5 {
            for z in 0..5 {
                chunk.set(x, 7, z, BlockId::SAND);
            }
        }

        // A tiny tree near the center.
        let (tx, tz) = (8, 8);
        for y in 8..12 {
            chunk.set(tx, y, tz, BlockId::LOG);
        }
        for y in 10..13 {
            let radius = if y == 12 { 1 } else { 2 };
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let lx = tx as isize + dx;
                    let lz = tz as isize + dz;
                    if lx == tx as isize && lz == tz as isize && y < 12 {
                        continue;
                    }
                    if lx >= 0 && lz >= 0 && (lx as usize) < CHUNK_SIZE && (lz as usize) < CHUNK_SIZE {
                        chunk.set(lx as usize, y, lz as usize, BlockId::LEAVES);
                    }
                }
            }
        }

        // A small cobblestone/plank structure to show off face culling.
        for x in 12..15 {
            for z in 2..5 {
                chunk.set(x, 8, z, BlockId::COBBLESTONE);
                chunk.set(x, 9, z, BlockId::WOOD_PLANKS);
            }
        }

        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_freshly_allocated_chunks_light_is_unset_zero_in_bounds() {
        // Zero here means "no light data received yet" (an allocated
        // but never-lit section) — distinct from a section that's
        // absent entirely, where the *column* level defaults sky
        // light to MAX_LIGHT (open sky) instead; see chunk_column.rs.
        let chunk = Chunk::empty();
        assert_eq!(chunk.block_light(1, 2, 3), 0);
        assert_eq!(chunk.sky_light(1, 2, 3), 0);
    }

    #[test]
    fn out_of_bounds_sky_light_defaults_to_max_even_within_a_real_chunk() {
        let chunk = Chunk::empty();
        assert_eq!(chunk.sky_light(99, 0, 0), MAX_LIGHT);
    }

    #[test]
    fn set_and_get_light_round_trips() {
        let mut chunk = Chunk::empty();
        chunk.set_block_light(1, 2, 3, 7);
        chunk.set_sky_light(1, 2, 3, 4);
        assert_eq!(chunk.block_light(1, 2, 3), 7);
        assert_eq!(chunk.sky_light(1, 2, 3), 4);
        // A neighboring cell is untouched.
        assert_eq!(chunk.block_light(1, 2, 4), 0);
    }

    #[test]
    fn light_levels_are_clamped_to_max_light() {
        let mut chunk = Chunk::empty();
        chunk.set_block_light(0, 0, 0, 200);
        assert_eq!(chunk.block_light(0, 0, 0), MAX_LIGHT);
    }

    #[test]
    fn out_of_bounds_light_queries_use_the_same_defaults() {
        let chunk = Chunk::empty();
        assert_eq!(chunk.block_light(99, 0, 0), 0);
        assert_eq!(chunk.sky_light(99, 0, 0), MAX_LIGHT);
    }

    #[test]
    fn hardcoded_demo_is_lit_as_full_outdoor_daylight() {
        let chunk = Chunk::hardcoded_demo();
        assert_eq!(chunk.sky_light(0, 0, 0), MAX_LIGHT);
        assert_eq!(chunk.sky_light(15, 15, 15), MAX_LIGHT);
        assert_eq!(chunk.block_light(8, 8, 8), 0);
    }
}
