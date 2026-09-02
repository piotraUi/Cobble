use std::collections::HashMap;

use crate::block::BlockId;
use crate::chunk::{CHUNK_SIZE, MAX_LIGHT};
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

    /// Returns `(block_light, sky_light)`, each 0-15. Positions with no
    /// loaded column default to open sky (0 block light, full sky
    /// light) rather than darkness — the common case is the edge of
    /// the loaded world, not an actual cave.
    pub fn get_light(&self, world_x: i32, world_y: i32, world_z: i32) -> (u8, u8) {
        if world_y < 0 {
            return (0, MAX_LIGHT);
        }
        let chunk_x = world_x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = world_z.div_euclid(CHUNK_SIZE as i32);
        let local_x = world_x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_SIZE as i32) as usize;

        match self.column(chunk_x, chunk_z) {
            Some(column) => column.get_light(local_x, world_y as usize, local_z),
            None => (0, MAX_LIGHT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_column::ChunkColumn;

    #[test]
    fn unloaded_area_defaults_to_open_sky_light() {
        let world = World::new();
        assert_eq!(world.get_light(0, 64, 0), (0, MAX_LIGHT));
    }

    #[test]
    fn below_the_world_defaults_to_open_sky_light() {
        let world = World::new();
        assert_eq!(world.get_light(0, -1, 0), (0, MAX_LIGHT));
    }

    #[test]
    fn loaded_column_returns_its_real_light_values() {
        let mut world = World::new();
        let mut column = ChunkColumn::empty(0, 0);
        column.set_block_light(1, 20, 1, 9);
        column.set_sky_light(1, 20, 1, 2);
        world.insert_column(column);

        assert_eq!(world.get_light(1, 20, 1), (9, 2));
    }
}
