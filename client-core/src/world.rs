use std::collections::HashMap;

use crate::block::BlockId;
use crate::chunk::CHUNK_SIZE;
use crate::chunk_column::ChunkColumn;

/// All loaded chunk columns, keyed by chunk coordinates (world position
/// divided by 16, floored).
#[derive(Default)]
pub struct World {
    columns: HashMap<(i32, i32), ChunkColumn>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_column(&mut self, column: ChunkColumn) {
        self.columns.insert((column.chunk_x, column.chunk_z), column);
    }

    pub fn remove_column(&mut self, chunk_x: i32, chunk_z: i32) {
        self.columns.remove(&(chunk_x, chunk_z));
    }

    pub fn column(&self, chunk_x: i32, chunk_z: i32) -> Option<&ChunkColumn> {
        self.columns.get(&(chunk_x, chunk_z))
    }

    pub fn columns(&self) -> impl Iterator<Item = &ChunkColumn> {
        self.columns.values()
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn set_block(&mut self, world_x: i32, world_y: i32, world_z: i32, block: BlockId) {
        if world_y < 0 {
            return;
        }
        let chunk_x = world_x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = world_z.div_euclid(CHUNK_SIZE as i32);
        let local_x = world_x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_SIZE as i32) as usize;

        if let Some(column) = self.columns.get_mut(&(chunk_x, chunk_z)) {
            column.set_block(local_x, world_y as usize, local_z, block);
        }
    }

    pub fn get_block(&self, world_x: i32, world_y: i32, world_z: i32) -> BlockId {
        if world_y < 0 {
            return BlockId::AIR;
        }
        let chunk_x = world_x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = world_z.div_euclid(CHUNK_SIZE as i32);
        let local_x = world_x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_SIZE as i32) as usize;

        match self.column(chunk_x, chunk_z) {
            Some(column) => column.get_block(local_x, world_y as usize, local_z),
            None => BlockId::AIR,
        }
    }
}
