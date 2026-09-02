use client_core::chunk_column::WORLD_HEIGHT;
use client_core::{Chunk, World, CHUNK_SIZE};

use crate::vertex::Vertex;

/// The 6 axis-aligned cube faces, each described by its outward normal,
/// a simple directional shading factor (stand-in for real block/sky
/// light until the protocol crate delivers real light data), and the
/// 4 corner offsets (in CCW winding order when viewed from outside).
struct Face {
    normal: [f32; 3],
    shade: f32,
    corners: [[f32; 3]; 4],
}

const FACES: [Face; 6] = [
    // +X (east)
    Face {
        normal: [1.0, 0.0, 0.0],
        shade: 0.8,
        corners: [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
    },
    // -X (west)
    Face {
        normal: [-1.0, 0.0, 0.0],
        shade: 0.8,
        corners: [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
        ],
    },
    // +Y (up)
    Face {
        normal: [0.0, 1.0, 0.0],
        shade: 1.0,
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
    },
    // -Y (down)
    Face {
        normal: [0.0, -1.0, 0.0],
        shade: 0.5,
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
    },
    // +Z (south)
    Face {
        normal: [0.0, 0.0, 1.0],
        shade: 0.9,
        corners: [
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ],
    },
    // -Z (north)
    Face {
        normal: [0.0, 0.0, -1.0],
        shade: 0.7,
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
    },
];

/// Neighbor offsets matching `FACES`' order (+X, -X, +Y, -Y, +Z, -Z).
const NEIGHBOR_OFFSETS: [(isize, isize, isize); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

/// Builds a mesh for `chunk` using simple per-face culling: a face is
/// only emitted when the block on the other side of it is missing or
/// non-opaque, so fully-buried faces never reach the GPU. This is not
/// greedy meshing yet, just the "at least" bar from the roadmap.
pub fn mesh_chunk(chunk: &Chunk) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let block = chunk.get(x, y, z);
                if block.is_air() {
                    continue;
                }

                let color = block.debug_color();

                for (face, offset) in FACES.iter().zip(NEIGHBOR_OFFSETS.iter()) {
                    let (nx, ny, nz) = (
                        x as isize + offset.0,
                        y as isize + offset.1,
                        z as isize + offset.2,
                    );

                    let neighbor_opaque = if nx < 0 || ny < 0 || nz < 0 {
                        false
                    } else {
                        chunk.get(nx as usize, ny as usize, nz as usize).is_opaque()
                    };

                    if neighbor_opaque {
                        continue;
                    }

                    let base_index = vertices.len() as u32;
                    let shaded_color = [
                        color[0] * face.shade,
                        color[1] * face.shade,
                        color[2] * face.shade,
                    ];

                    for corner in face.corners.iter() {
                        vertices.push(Vertex {
                            position: [
                                x as f32 + corner[0],
                                y as f32 + corner[1],
                                z as f32 + corner[2],
                            ],
                            normal: face.normal,
                            color: shaded_color,
                        });
                    }

                    indices.extend_from_slice(&[
                        base_index,
                        base_index + 1,
                        base_index + 2,
                        base_index,
                        base_index + 2,
                        base_index + 3,
                    ]);
                }
            }
        }
    }

    (vertices, indices)
}

/// Builds one combined mesh for every currently loaded chunk column in
/// `world`, face-culling against neighbors across chunk boundaries too
/// (via `World::get_block`, unlike `mesh_chunk`'s section-local lookup).
/// Rebuilding the whole world mesh on every chunk load is not cheap,
/// but it's the simplest thing that's correct — see roadmap step 7 for
/// per-chunk incremental meshing.
pub fn mesh_world(world: &World) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for column in world.columns() {
        let base_x = column.chunk_x * CHUNK_SIZE as i32;
        let base_z = column.chunk_z * CHUNK_SIZE as i32;

        for y in 0..WORLD_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let block = column.get_block(x, y, z);
                    if block.is_air() {
                        continue;
                    }

                    let world_x = base_x + x as i32;
                    let world_z = base_z + z as i32;
                    let color = block.debug_color();

                    for (face, offset) in FACES.iter().zip(NEIGHBOR_OFFSETS.iter()) {
                        let neighbor_opaque = world
                            .get_block(world_x + offset.0 as i32, y as i32 + offset.1 as i32, world_z + offset.2 as i32)
                            .is_opaque();
                        if neighbor_opaque {
                            continue;
                        }

                        let base_index = vertices.len() as u32;
                        let shaded_color = [
                            color[0] * face.shade,
                            color[1] * face.shade,
                            color[2] * face.shade,
                        ];

                        for corner in face.corners.iter() {
                            vertices.push(Vertex {
                                position: [
                                    world_x as f32 + corner[0],
                                    y as f32 + corner[1],
                                    world_z as f32 + corner[2],
                                ],
                                normal: face.normal,
                                color: shaded_color,
                            });
                        }

                        indices.extend_from_slice(&[
                            base_index,
                            base_index + 1,
                            base_index + 2,
                            base_index,
                            base_index + 2,
                            base_index + 3,
                        ]);
                    }
                }
            }
        }
    }

    (vertices, indices)
}
