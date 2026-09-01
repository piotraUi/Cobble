use crate::block::BlockId;

pub const CHUNK_SIZE: usize = 16;

/// A single 16x16x16 chunk section, matching the Minecraft 1.8.9 chunk
/// section size used on the wire (see `protocol`'s future Chunk Data
/// packet handling).
pub struct Chunk {
    blocks: [BlockId; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: [BlockId::AIR; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
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

    /// Builds a small hardcoded demo terrain: a few stone/dirt/grass
    /// layers with a sand patch and a single tree, used before real
    /// network chunk data is available (see roadmap step 2).
    pub fn hardcoded_demo() -> Self {
        let mut chunk = Self::empty();

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
