/// A single voxel block type. Values line up with the Minecraft 1.8.9
/// numeric block IDs for the small set we know about so far; unknown IDs
/// still round-trip through `raw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockId(pub u16);

impl BlockId {
    pub const AIR: BlockId = BlockId(0);
    pub const STONE: BlockId = BlockId(1);
    pub const GRASS: BlockId = BlockId(2);
    pub const DIRT: BlockId = BlockId(3);
    pub const COBBLESTONE: BlockId = BlockId(4);
    pub const WOOD_PLANKS: BlockId = BlockId(5);
    pub const BEDROCK: BlockId = BlockId(7);
    pub const SAND: BlockId = BlockId(12);
    pub const LOG: BlockId = BlockId(17);
    pub const LEAVES: BlockId = BlockId(18);

    #[inline]
    pub fn is_air(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn is_opaque(self) -> bool {
        // Every block we know about right now is a full opaque cube;
        // this will need per-block shape data once slabs/stairs/etc. land.
        !self.is_air()
    }

    /// Placeholder color used until real block textures / atlas UVs exist.
    /// Returns linear RGB in [0, 1].
    pub fn debug_color(self) -> [f32; 3] {
        match self {
            BlockId::STONE => [0.5, 0.5, 0.5],
            BlockId::GRASS => [0.33, 0.62, 0.28],
            BlockId::DIRT => [0.46, 0.33, 0.22],
            BlockId::COBBLESTONE => [0.4, 0.4, 0.4],
            BlockId::WOOD_PLANKS => [0.65, 0.5, 0.32],
            BlockId::BEDROCK => [0.15, 0.15, 0.15],
            BlockId::SAND => [0.85, 0.8, 0.6],
            BlockId::LOG => [0.4, 0.29, 0.16],
            BlockId::LEAVES => [0.2, 0.45, 0.15],
            _ => [1.0, 0.0, 1.0], // magenta = "unknown block"
        }
    }
}
