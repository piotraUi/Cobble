use client_core::chunk::MAX_LIGHT;
use client_core::chunk_column::WORLD_HEIGHT;
use client_core::{BlockId, Chunk, World, CHUNK_SIZE};
use texturepacks::TextureAtlas;

use crate::block_textures::{face_uv, FaceKind};
use crate::vertex::Vertex;

/// A face's brightness never drops below this even in total darkness —
/// a real voxel/light propagation engine (see roadmap step 7's other
/// items) would make pitch-black caves genuinely readable-as-empty,
/// but until then a fully unlit face being pure black just looks like
/// a rendering bug, so a small ambient floor keeps it legible.
const MIN_BRIGHTNESS: f32 = 0.05;

/// Combines block + sky light (each 0-15, sky already time-of-day
/// adjusted server-side... except we don't track time of day yet, so
/// this treats sky light as always full daytime brightness — see
/// roadmap step 7) into the [MIN_BRIGHTNESS, 1.0] multiplier `push_face`
/// applies on top of its existing per-face directional shade.
fn light_brightness(block_light: u8, sky_light: u8) -> f32 {
    let level = block_light.max(sky_light).min(MAX_LIGHT) as f32 / MAX_LIGHT as f32;
    level.max(MIN_BRIGHTNESS)
}

/// The 6 axis-aligned cube faces, each described by its outward normal,
/// a simple directional shading factor (stand-in for real block/sky
/// light until the protocol crate delivers real light data), which
/// texture (top/bottom/side) it should sample, and the 4 corner
/// offsets (in CCW winding order when viewed from outside).
struct Face {
    normal: [f32; 3],
    shade: f32,
    kind: FaceKind,
    corners: [[f32; 3]; 4],
    /// True when `corners` walks its 4 points clockwise as seen from
    /// outside the cube (along `normal`) instead of the counter-
    /// clockwise order the pipeline's `front_face: Ccw` + backface
    /// culling expects. Rather than reorder `corners` (which would also
    /// have to be paired with a different `UV_CORNERS` slice to avoid
    /// mirroring the texture), `push_face` just reads the same 4
    /// vertices into their triangles in the opposite order for these
    /// faces, which flips the winding without touching position or UV
    /// data at all.
    reversed_winding: bool,
}

/// Texture-space corners matching `Face::corners`' winding order, the
/// same for every face — Minecraft-style block textures tile/mirror
/// fine under any of the 4 rotations, so one fixed mapping is enough.
const UV_CORNERS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

const FACES: [Face; 6] = [
    // +X (east)
    Face {
        normal: [1.0, 0.0, 0.0],
        shade: 0.8,
        kind: FaceKind::Side,
        corners: [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        reversed_winding: true,
    },
    // -X (west)
    Face {
        normal: [-1.0, 0.0, 0.0],
        shade: 0.8,
        kind: FaceKind::Side,
        corners: [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
        ],
        reversed_winding: true,
    },
    // +Y (up)
    Face {
        normal: [0.0, 1.0, 0.0],
        shade: 1.0,
        kind: FaceKind::Top,
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        reversed_winding: false,
    },
    // -Y (down)
    Face {
        normal: [0.0, -1.0, 0.0],
        shade: 0.5,
        kind: FaceKind::Bottom,
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        reversed_winding: false,
    },
    // +Z (south)
    Face {
        normal: [0.0, 0.0, 1.0],
        shade: 0.9,
        kind: FaceKind::Side,
        corners: [
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ],
        reversed_winding: true,
    },
    // -Z (north)
    Face {
        normal: [0.0, 0.0, -1.0],
        shade: 0.7,
        kind: FaceKind::Side,
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        reversed_winding: true,
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

/// Emits one face's two triangles into `vertices`/`indices`, sampling
/// `atlas` for the UV rect and applying `face.shade` (a directional
/// stand-in for ambient occlusion) times `brightness` (the real
/// block/sky light at the face — see `light_brightness`) as the
/// per-vertex color multiplier.
#[allow(clippy::too_many_arguments)]
fn push_face(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    atlas: &TextureAtlas,
    block: BlockId,
    face: &Face,
    origin: [f32; 3],
    brightness: f32,
) {
    let (u0, v0, u1, v1) = face_uv(atlas, block, face.kind);
    let combined = face.shade * brightness;
    let shade = [combined, combined, combined];

    let base_index = vertices.len() as u32;
    for (corner, uv_corner) in face.corners.iter().zip(UV_CORNERS.iter()) {
        vertices.push(Vertex {
            position: [
                origin[0] + corner[0],
                origin[1] + corner[1],
                origin[2] + corner[2],
            ],
            normal: face.normal,
            color: shade,
            uv: [u0 + uv_corner[0] * (u1 - u0), v0 + uv_corner[1] * (v1 - v0)],
        });
    }

    if face.reversed_winding {
        indices.extend_from_slice(&[
            base_index,
            base_index + 2,
            base_index + 1,
            base_index,
            base_index + 3,
            base_index + 2,
        ]);
    } else {
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

/// Builds a mesh for `chunk` using simple per-face culling: a face is
/// only emitted when the block on the other side of it is missing or
/// non-opaque, so fully-buried faces never reach the GPU. This is not
/// greedy meshing yet, just the "at least" bar from the roadmap.
pub fn mesh_chunk(chunk: &Chunk, atlas: &TextureAtlas) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let block = chunk.get(x, y, z);
                if block.is_air() {
                    continue;
                }

                for (face, offset) in FACES.iter().zip(NEIGHBOR_OFFSETS.iter()) {
                    let (nx, ny, nz) = (
                        x as isize + offset.0,
                        y as isize + offset.1,
                        z as isize + offset.2,
                    );

                    let in_bounds = nx >= 0 && ny >= 0 && nz >= 0 && (nx as usize) < CHUNK_SIZE && (ny as usize) < CHUNK_SIZE && (nz as usize) < CHUNK_SIZE;
                    let (neighbor_opaque, block_light, sky_light) = if in_bounds {
                        let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
                        (
                            chunk.get(nx, ny, nz).is_opaque(),
                            chunk.block_light(nx, ny, nz),
                            chunk.sky_light(nx, ny, nz),
                        )
                    } else {
                        // Outside this section — treat like the edge of
                        // the loaded world (see World::get_light).
                        (false, 0, MAX_LIGHT)
                    };

                    if neighbor_opaque {
                        continue;
                    }

                    push_face(
                        &mut vertices,
                        &mut indices,
                        atlas,
                        block,
                        face,
                        [x as f32, y as f32, z as f32],
                        light_brightness(block_light, sky_light),
                    );
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
pub fn mesh_world(world: &World, atlas: &TextureAtlas) -> (Vec<Vertex>, Vec<u32>) {
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

                    for (face, offset) in FACES.iter().zip(NEIGHBOR_OFFSETS.iter()) {
                        let (nx, ny, nz) = (world_x + offset.0 as i32, y as i32 + offset.1 as i32, world_z + offset.2 as i32);
                        if world.get_block(nx, ny, nz).is_opaque() {
                            continue;
                        }

                        let (block_light, sky_light) = world.get_light(nx, ny, nz);
                        push_face(
                            &mut vertices,
                            &mut indices,
                            atlas,
                            block,
                            face,
                            [world_x as f32, y as f32, world_z as f32],
                            light_brightness(block_light, sky_light),
                        );
                    }
                }
            }
        }
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use client_core::BlockId;

    /// Every face's *actual* triangle winding (computed from the
    /// vertices it emits, not the `Face::normal` field, which is only
    /// ever a label) must produce an outward-facing normal via the
    /// right-hand rule — otherwise, with `front_face: Ccw` + backface
    /// culling in the GPU pipeline, the face renders only when viewed
    /// from *inside* the block instead of outside. This regression-
    /// tests the real device bug where all 4 side faces (+X/-X/+Z/-Z)
    /// had reversed winding and were invisible from outside.
    #[test]
    fn every_faces_first_triangle_winds_outward() {
        for face in &FACES {
            let a = face.corners[0];
            let b = face.corners[1];
            let c = face.corners[2];
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];

            // The triangle actually drawn is (A,C,B) when reversed_winding
            // is set — see push_face — so cross the same two edges the
            // GPU would, in the order it would.
            let (u, v) = if face.reversed_winding { (ac, ab) } else { (ab, ac) };
            let cross = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];

            for axis in 0..3 {
                assert!(
                    (cross[axis] - face.normal[axis]).abs() < 1e-6,
                    "face with normal {:?} winds to {:?} instead (reversed_winding={})",
                    face.normal,
                    cross,
                    face.reversed_winding
                );
            }
        }
    }

    #[test]
    fn single_block_produces_6_culled_faces_all_within_its_own_atlas_tile() {
        let atlas = texturepacks::build_fallback_atlas();
        let mut chunk = Chunk::empty();
        chunk.set(5, 5, 5, BlockId::STONE);

        let (vertices, indices) = mesh_chunk(&chunk, &atlas);

        assert_eq!(indices.len(), 6 * 6, "6 faces * 2 triangles * 3 indices");
        assert_eq!(vertices.len(), 6 * 4, "6 faces * 4 corners, unwelded");

        let (u0, v0, u1, v1) = atlas.uv(&texturepacks::block_atlas_key("stone")).unwrap();
        for vertex in &vertices {
            assert!(vertex.uv[0] >= u0 - 1e-6 && vertex.uv[0] <= u1 + 1e-6);
            assert!(vertex.uv[1] >= v0 - 1e-6 && vertex.uv[1] <= v1 + 1e-6);
        }
    }

    #[test]
    fn buried_block_contributes_no_faces() {
        let atlas = texturepacks::build_fallback_atlas();
        let mut chunk = Chunk::empty();
        // A center block fully surrounded by 6 opaque neighbors (each
        // neighbor touches only the center, not each other, so every
        // neighbor is itself missing exactly the one face pointing at
        // the center — 5 exposed faces apiece).
        chunk.set(5, 5, 5, BlockId::STONE);
        for (dx, dy, dz) in [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            chunk.set((5 + dx) as usize, (5 + dy) as usize, (5 + dz) as usize, BlockId::STONE);
        }

        let (vertices, indices) = mesh_chunk(&chunk, &atlas);
        let expected_faces = 6 * 5; // 6 neighbors * 5 exposed faces each; center contributes 0
        assert_eq!(vertices.len(), expected_faces * 4);
        assert_eq!(indices.len(), expected_faces * 6);
    }

    #[test]
    fn light_brightness_uses_the_brighter_of_block_and_sky_light() {
        assert!((light_brightness(0, 0) - MIN_BRIGHTNESS).abs() < 1e-6);
        assert!((light_brightness(MAX_LIGHT, 0) - 1.0).abs() < 1e-6);
        assert!((light_brightness(0, MAX_LIGHT) - 1.0).abs() < 1e-6);
        assert!((light_brightness(MAX_LIGHT, MAX_LIGHT) - 1.0).abs() < 1e-6);

        let half = light_brightness(MAX_LIGHT / 2, 0);
        assert!(half > MIN_BRIGHTNESS && half < 1.0);
    }

    #[test]
    fn brightness_never_drops_below_the_ambient_floor() {
        assert_eq!(light_brightness(0, 0), MIN_BRIGHTNESS);
    }

    #[test]
    fn a_fully_lit_block_meshes_brighter_than_a_fully_dark_one() {
        let atlas = texturepacks::build_fallback_atlas();

        let mut lit = Chunk::empty();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    lit.set_sky_light(x, y, z, MAX_LIGHT);
                }
            }
        }
        lit.set(5, 5, 5, BlockId::STONE);
        let (lit_vertices, _) = mesh_chunk(&lit, &atlas);

        let mut dark = Chunk::empty(); // block/sky light both default to 0
        dark.set(5, 5, 5, BlockId::STONE);
        let (dark_vertices, _) = mesh_chunk(&dark, &atlas);

        assert_eq!(lit_vertices.len(), dark_vertices.len());
        for (lit_v, dark_v) in lit_vertices.iter().zip(dark_vertices.iter()) {
            for channel in 0..3 {
                assert!(
                    lit_v.color[channel] > dark_v.color[channel],
                    "expected lit vertex brighter than dark one: {:?} vs {:?}",
                    lit_v.color,
                    dark_v.color
                );
            }
        }
    }
}
