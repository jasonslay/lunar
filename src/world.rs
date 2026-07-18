use glam::Vec2;

use crate::physics::PIXELS_PER_METER;

pub const TERRAIN_POINTS: usize = 400;
pub const PAD_WIDTH: f32 = 16.0;
pub const PAD_SHOULDER_WIDTH: f32 = 10.0;
pub const PAD_SHOULDER_RISE: f32 = 2.5;
pub const PAD_REGION_FRAC: f32 = 0.35;
pub const SCREEN_WIDTH: f32 = 800.0;
pub const SCREEN_HEIGHT: f32 = 600.0;
pub const VIEW_WIDTH: f32 = SCREEN_WIDTH / PIXELS_PER_METER;
/// Downrange edge of the map (spawn side).
pub const WORLD_WIDTH: f32 = 760.0;
/// Extra playable runway west of the landing pad for overshoot recovery.
pub const PAST_TARGET_WIDTH: f32 = 50.0;
pub const WORLD_MIN_X: f32 = -PAST_TARGET_WIDTH;
/// Terrain extends past playable bounds so the surface line fills the viewport.
pub const MAP_EDGE_BLEED_LEFT: f32 = 20.0;
pub const MAP_EDGE_BLEED_RIGHT: f32 = 40.0;
pub const TERRAIN_MIN_X: f32 = WORLD_MIN_X - MAP_EDGE_BLEED_LEFT;
pub const TERRAIN_MAX_X: f32 = WORLD_WIDTH + MAP_EDGE_BLEED_RIGHT;
pub const WORLD_HEIGHT: f32 = SCREEN_HEIGHT / PIXELS_PER_METER;
pub const TERRAIN_MARKER_SPACING: f32 = 10.0;
pub const STAR_COUNT: usize = 350;

#[derive(Clone, Copy)]
pub struct Star {
    pub nx: f32,
    pub ny: f32,
    pub brightness: f32,
}

pub struct World {
    pub terrain: Vec<Vec2>,
    pub stars: Vec<Star>,
    pub pad_start_x: f32,
    pub pad_end_x: f32,
    pub pad_center_x: f32,
    pub pad_y: f32,
}

impl World {
    fn generate_stars(seed: u64) -> Vec<Star> {
        let mut rng = star_rng(seed);
        let mut stars = Vec::with_capacity(STAR_COUNT);
        for _ in 0..STAR_COUNT {
            stars.push(Star {
                nx: rng(),
                ny: rng(),
                brightness: 0.4 + rng() * 0.6,
            });
        }
        stars
    }

    pub fn generate(seed: u64) -> Self {
        let mut terrain = Vec::with_capacity(TERRAIN_POINTS + 1);
        let span = TERRAIN_MAX_X - TERRAIN_MIN_X;
        let step = span / TERRAIN_POINTS as f32;
        let base_y = WORLD_HEIGHT - 10.0;

        for i in 0..=TERRAIN_POINTS {
            let x = if i == 0 {
                TERRAIN_MIN_X
            } else if i == TERRAIN_POINTS {
                TERRAIN_MAX_X
            } else {
                TERRAIN_MIN_X + i as f32 * step
            };
            let t = (x - TERRAIN_MIN_X) / span;

            let h = base_y
                - 3.75 * (t * 4.0 * std::f32::consts::PI + seed as f32 * 0.01).sin()
                - 1.875 * (t * 9.0 * std::f32::consts::PI + seed as f32 * 0.03).sin()
                - 1.0 * (t * 17.0 * std::f32::consts::PI).sin();

            terrain.push(Vec2::new(x, h));
        }

        let search_min_x = 8.0;
        let search_max_x = WORLD_WIDTH * PAD_REGION_FRAC;
        let mut pad_center_x = search_min_x;
        let mut pad_y = f32::INFINITY;
        for point in &terrain {
            if point.x >= search_min_x && point.x <= search_max_x && point.y < pad_y {
                pad_y = point.y;
                pad_center_x = point.x;
            }
        }
        let pad_half = PAD_WIDTH / 2.0;
        let pad_start_x = pad_center_x - pad_half;
        let pad_end_x = pad_center_x + pad_half;

        for point in terrain.iter_mut() {
            if point.x >= pad_start_x && point.x <= pad_end_x {
                point.y = pad_y;
            }
        }

        for point in terrain.iter_mut() {
            let dist_outside = if point.x < pad_start_x {
                pad_start_x - point.x
            } else if point.x > pad_end_x {
                point.x - pad_end_x
            } else {
                continue;
            };

            if dist_outside <= PAD_SHOULDER_WIDTH {
                let t = dist_outside / PAD_SHOULDER_WIDTH;
                let floor = pad_y + PAD_SHOULDER_RISE * (1.0 - t);
                point.y = point.y.max(floor);
            }
        }

        Self {
            terrain,
            stars: Self::generate_stars(seed),
            pad_start_x,
            pad_end_x,
            pad_center_x,
            pad_y,
        }
    }

    pub fn height_at(&self, x: f32) -> f32 {
        if x <= self.terrain[0].x {
            return self.terrain[0].y;
        }
        if x >= self.terrain[self.terrain.len() - 1].x {
            return self.terrain[self.terrain.len() - 1].y;
        }

        for window in self.terrain.windows(2) {
            let a = window[0];
            let b = window[1];
            if x >= a.x && x <= b.x {
                let t = (x - a.x) / (b.x - a.x);
                return a.y + t * (b.y - a.y);
            }
        }

        self.terrain[0].y
    }

    pub fn is_on_pad(&self, x: f32) -> bool {
        x >= self.pad_start_x && x <= self.pad_end_x
    }

    pub fn check_collision(&self, hull: &[Vec2]) -> bool {
        for vertex in hull {
            let ground = self.height_at(vertex.x);
            if vertex.y >= ground {
                return true;
            }
        }
        false
    }

    pub fn altitude(&self, x: f32, y: f32) -> f32 {
        let ground = self.height_at(x);
        (ground - y).max(0.0)
    }

    /// Minimum height above local terrain under the lander hull (radar-style AGL).
    pub fn clearance_above_terrain(&self, hull_world: &[Vec2]) -> f32 {
        hull_world
            .iter()
            .map(|v| self.altitude(v.x, v.y))
            .fold(f32::INFINITY, f32::min)
            .max(0.0)
    }
}

fn star_rng(seed: u64) -> impl FnMut() -> f32 {
    let mut state = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state as u32 as f32) / u32::MAX as f32
    }
}

#[cfg(test)]
mod star_tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn stars_should_be_visible_at_spawn() {
        let world = World::generate(42);
        assert_eq!(world.stars.len(), STAR_COUNT);

        let cam = Vec2::new(642.0, -14.0);
        let mut visible = 0usize;

        for star in &world.stars {
            let screen_x = star.nx * SCREEN_WIDTH;
            let screen_y = 4.0 + star.ny * (SCREEN_HEIGHT - 4.0);
            let world_x = cam.x + screen_x / PIXELS_PER_METER;
            let horizon_y = (world.height_at(world_x) - cam.y) * PIXELS_PER_METER;
            if horizon_y < 12.0 {
                continue;
            }
            if screen_y < horizon_y - 1.0 {
                visible += 1;
            }
        }

        assert!(visible > 100, "expected many visible stars, got {visible}");
    }
}

#[cfg(test)]
mod altitude_tests {
    use super::*;

    #[test]
    fn clearance_uses_local_terrain_not_pad_elevation() {
        let world = World::generate(42);
        let spawn_x = (world.pad_center_x + 480.0).clamp(8.0, WORLD_WIDTH - 8.0);
        let ground = world.height_at(spawn_x);
        let spawn_y = ground - 50.0;
        let pad_relative = world.pad_y - spawn_y;

        let agl = world.altitude(spawn_x, spawn_y);
        assert!((agl - 50.0).abs() < 0.01);

        if (ground - world.pad_y).abs() > 1.0 {
            assert!(
                (agl - pad_relative).abs() > 1.0,
                "agl should follow local ground ({agl}), not pad elevation ({pad_relative})"
            );
        }
    }
}
