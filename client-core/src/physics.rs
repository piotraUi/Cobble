//! Gravity + AABB-vs-voxel collision for the player, resolved one axis
//! at a time so the player slides along walls/edges instead of
//! stopping dead on diagonal movement. Values are tuned to feel
//! roughly like vanilla Minecraft (not measured against it).

use glam::Vec3;

use crate::world::World;

pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_HEIGHT: f32 = 1.8;
/// Camera height above the feet position, matching vanilla's eye height.
pub const EYE_HEIGHT: f32 = 1.62;

pub const WALK_SPEED: f32 = 4.3;
pub const SNEAK_SPEED_MULTIPLIER: f32 = 0.3;
const GRAVITY: f32 = 32.0;
const JUMP_VELOCITY: f32 = 9.0;
const TERMINAL_VELOCITY: f32 = 78.4;

/// A small inward epsilon so we don't treat blocks that only just touch
/// the player's bounding box (shared face, zero-width overlap) as a
/// collision.
const EPSILON: f32 = 1e-4;

#[derive(Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn from_feet_position(position: Vec3) -> Self {
        let half_width = PLAYER_WIDTH / 2.0;
        Self {
            min: Vec3::new(position.x - half_width, position.y, position.z - half_width),
            max: Vec3::new(
                position.x + half_width,
                position.y + PLAYER_HEIGHT,
                position.z + half_width,
            ),
        }
    }

    fn axis(&self, axis: Axis) -> (f32, f32) {
        match axis {
            Axis::X => (self.min.x, self.max.x),
            Axis::Y => (self.min.y, self.max.y),
            Axis::Z => (self.min.z, self.max.z),
        }
    }
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn others(self) -> (Axis, Axis) {
        match self {
            Axis::X => (Axis::Y, Axis::Z),
            Axis::Y => (Axis::X, Axis::Z),
            Axis::Z => (Axis::X, Axis::Y),
        }
    }
}

pub struct PlayerPhysics {
    /// Feet position (bottom-center of the player's bounding box) —
    /// same convention the 1.8.9 protocol uses for player position.
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
}

impl PlayerPhysics {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            on_ground: false,
        }
    }

    pub fn eye_position(&self) -> Vec3 {
        self.position + Vec3::new(0.0, EYE_HEIGHT, 0.0)
    }

    /// Advances the simulation by `dt` seconds. `wish_dir` is a
    /// (possibly zero) horizontal direction vector, already
    /// camera-relative and NOT necessarily normalized — this function
    /// normalizes it before scaling by walk speed.
    pub fn update(&mut self, world: &World, wish_dir: Vec3, jump: bool, sneaking: bool, dt: f32) {
        let mut speed = WALK_SPEED;
        if sneaking {
            speed *= SNEAK_SPEED_MULTIPLIER;
        }
        let horizontal = Vec3::new(wish_dir.x, 0.0, wish_dir.z).normalize_or_zero() * speed;
        self.velocity.x = horizontal.x;
        self.velocity.z = horizontal.z;

        if self.on_ground && jump {
            self.velocity.y = JUMP_VELOCITY;
        }

        self.velocity.y = (self.velocity.y - GRAVITY * dt).max(-TERMINAL_VELOCITY);

        self.on_ground = false;
        self.move_and_collide(world, Axis::Y, self.velocity.y * dt);
        self.move_and_collide(world, Axis::X, self.velocity.x * dt);
        self.move_and_collide(world, Axis::Z, self.velocity.z * dt);
    }

    fn move_and_collide(&mut self, world: &World, axis: Axis, delta: f32) {
        if delta == 0.0 {
            return;
        }

        let aabb = Aabb::from_feet_position(self.position);
        let (other_a, other_b) = axis.others();
        let (a_min, a_max) = aabb.axis(other_a);
        let (b_min, b_max) = aabb.axis(other_b);
        let (axis_min, axis_max) = aabb.axis(axis);

        // Padded by SEARCH_MARGIN so a body resting exactly on a block
        // boundary (delta ~ 0, sweep collapsing to a single float) still
        // finds the block it's resting on — otherwise `floor()` can push
        // the search range just past it and `on_ground` flickers.
        const SEARCH_MARGIN: f32 = 1e-3;
        let sweep_min = axis_min.min(axis_min + delta) - SEARCH_MARGIN;
        let sweep_max = axis_max.max(axis_max + delta) + SEARCH_MARGIN;

        let block_range = |lo: f32, hi: f32| (lo.floor() as i32)..(hi.ceil() as i32);

        let (x_range, y_range, z_range) = match axis {
            Axis::X => (block_range(sweep_min, sweep_max), block_range(a_min, a_max), block_range(b_min, b_max)),
            Axis::Y => (block_range(a_min, a_max), block_range(sweep_min, sweep_max), block_range(b_min, b_max)),
            Axis::Z => (block_range(a_min, a_max), block_range(b_min, b_max), block_range(sweep_min, sweep_max)),
        };

        let mut allowed = delta;
        let mut collided = false;

        for bx in x_range {
            for by in y_range.clone() {
                for bz in z_range.clone() {
                    if !world.get_block(bx, by, bz).is_opaque() {
                        continue;
                    }
                    let (block_axis_min, block_axis_max) = match axis {
                        Axis::X => (bx as f32, bx as f32 + 1.0),
                        Axis::Y => (by as f32, by as f32 + 1.0),
                        Axis::Z => (bz as f32, bz as f32 + 1.0),
                    };

                    // Only a real obstacle if it overlaps on the two
                    // axes we're NOT currently moving along.
                    let (block_a_min, block_a_max) = match other_a {
                        Axis::X => (bx as f32, bx as f32 + 1.0),
                        Axis::Y => (by as f32, by as f32 + 1.0),
                        Axis::Z => (bz as f32, bz as f32 + 1.0),
                    };
                    let (block_b_min, block_b_max) = match other_b {
                        Axis::X => (bx as f32, bx as f32 + 1.0),
                        Axis::Y => (by as f32, by as f32 + 1.0),
                        Axis::Z => (bz as f32, bz as f32 + 1.0),
                    };
                    if a_max <= block_a_min + EPSILON || a_min >= block_a_max - EPSILON {
                        continue;
                    }
                    if b_max <= block_b_min + EPSILON || b_min >= block_b_max - EPSILON {
                        continue;
                    }

                    if delta > 0.0 {
                        let limit = block_axis_min - axis_max;
                        if limit >= 0.0 && limit < allowed {
                            allowed = limit;
                            collided = true;
                        }
                    } else {
                        let limit = block_axis_max - axis_min;
                        if limit <= 0.0 && limit > allowed {
                            allowed = limit;
                            collided = true;
                        }
                    }
                }
            }
        }

        match axis {
            Axis::X => self.position.x += allowed,
            Axis::Y => self.position.y += allowed,
            Axis::Z => self.position.z += allowed,
        }

        if collided {
            match axis {
                Axis::Y => {
                    if delta < 0.0 {
                        self.on_ground = true;
                    }
                    self.velocity.y = 0.0;
                }
                Axis::X => self.velocity.x = 0.0,
                Axis::Z => self.velocity.z = 0.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockId;
    use crate::chunk_column::ChunkColumn;

    /// A single flat 16x16 stone floor at y=0..1 (world y=0), everything
    /// else air, in one chunk column at (0, 0).
    fn flat_world() -> World {
        let mut world = World::new();
        let mut column = ChunkColumn::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                column.set_block(x, 0, z, BlockId::STONE);
            }
        }
        world.insert_column(column);
        world
    }

    fn step(physics: &mut PlayerPhysics, world: &World, seconds: f32) {
        // Small fixed steps so a single call never tunnels through a
        // block at high fall speed, matching how the real game loop
        // calls update() every frame.
        let mut remaining = seconds;
        while remaining > 0.0 {
            let dt = remaining.min(1.0 / 60.0);
            physics.update(world, Vec3::ZERO, false, false, dt);
            remaining -= dt;
        }
    }

    #[test]
    fn falls_and_lands_on_the_ground() {
        let world = flat_world();
        let mut physics = PlayerPhysics::new(Vec3::new(8.0, 10.0, 8.0));

        step(&mut physics, &world, 3.0);

        assert!(physics.on_ground, "should have landed on the floor");
        assert!(
            (physics.position.y - 1.0).abs() < 1e-3,
            "feet should rest on top of the floor (y=1.0), got {}",
            physics.position.y
        );
        assert_eq!(physics.velocity.y, 0.0);
    }

    #[test]
    fn jump_leaves_the_ground_then_lands_again() {
        let world = flat_world();
        let mut physics = PlayerPhysics::new(Vec3::new(8.0, 1.0, 8.0));
        physics.on_ground = true;

        physics.update(&world, Vec3::ZERO, true, false, 1.0 / 60.0);
        assert!(physics.velocity.y > 0.0, "jump should give upward velocity");
        assert!(!physics.on_ground, "should leave the ground immediately after jumping");

        step(&mut physics, &world, 3.0);
        assert!(physics.on_ground, "should come back down and land again");
        assert!((physics.position.y - 1.0).abs() < 1e-3);
    }

    #[test]
    fn horizontal_movement_is_blocked_by_a_wall() {
        let mut column = ChunkColumn::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                column.set_block(x, 0, z, BlockId::STONE);
            }
        }
        for y in 1..4 {
            column.set_block(9, y, 8, BlockId::STONE);
        }
        let mut world = World::new();
        world.insert_column(column);

        let mut physics = PlayerPhysics::new(Vec3::new(8.0, 1.0, 8.0));
        physics.on_ground = true;

        for _ in 0..120 {
            physics.update(&world, Vec3::new(1.0, 0.0, 0.0), false, false, 1.0 / 60.0);
        }

        // The wall's west face is at x=9; the player's half-width is 0.3,
        // so its center should stop at x=8.7, never reaching the wall.
        assert!(
            physics.position.x < 8.7 + 1e-3,
            "player should be stopped by the wall, got x={}",
            physics.position.x
        );
    }

    #[test]
    fn no_movement_when_wish_dir_is_zero_and_on_ground() {
        let world = flat_world();
        let mut physics = PlayerPhysics::new(Vec3::new(8.0, 1.0, 8.0));
        physics.on_ground = true;

        physics.update(&world, Vec3::ZERO, false, false, 1.0 / 60.0);

        assert_eq!(physics.position.x, 8.0);
        assert_eq!(physics.position.z, 8.0);
    }
}
