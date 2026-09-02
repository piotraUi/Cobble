use crate::block::BlockId;
use crate::chunk::{Chunk, CHUNK_SIZE};

/// Number of 16x16x16 sections stacked in a 1.8.9 chunk column
/// (0..256 world height).
pub const SECTIONS_PER_COLUMN: usize = 16;
pub const WORLD_HEIGHT: usize = SECTIONS_PER_COLUMN * CHUNK_SIZE;

/// A vertical stack of up to 16 chunk sections at a given (chunk_x,
/// chunk_z), matching how the Minecraft 1.8.9 Chunk Data packet is
/// laid out (a bitmask selects which of the 16 sections are present;
/// missing sections are all-air).
pub struct ChunkColumn {
    pub chunk_x: i32,
    pub chunk_z: i32,
    sections: [Option<Box<Chunk>>; SECTIONS_PER_COLUMN],
}

impl ChunkColumn {
    pub fn empty(chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            chunk_x,
            chunk_z,
            sections: Default::default(),
        }
    }

    pub fn section(&self, section_y: usize) -> Option<&Chunk> {
        self.sections.get(section_y)?.as_deref()
    }

    pub fn section_mut(&mut self, section_y: usize) -> &mut Chunk {
        self.sections[section_y].get_or_insert_with(|| Box::new(Chunk::empty()))
    }

    pub fn set_section(&mut self, section_y: usize, chunk: Chunk) {
        self.sections[section_y] = Some(Box::new(chunk));
    }

    /// `x`/`z` are local to the column (0..16), `y` is world height (0..256).
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockId {
        if y >= WORLD_HEIGHT {
            return BlockId::AIR;
        }
        let section_y = y / CHUNK_SIZE;
        match self.section(section_y) {
            Some(section) => section.get(x, y % CHUNK_SIZE, z),
            None => BlockId::AIR,
        }
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockId) {
        if y >= WORLD_HEIGHT {
            return;
        }
        let section_y = y / CHUNK_SIZE;
        self.section_mut(section_y)
            .set(x, y % CHUNK_SIZE, z, block);
    }
}
