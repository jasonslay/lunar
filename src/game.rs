use bevy::prelude::*;
use glam::Vec2;

use crate::autopilot;
use crate::input::{thrust_from_input, ThrustInput};
use crate::lander::{Lander, FUEL_BURN_RATE, FUEL_CAPACITY};
use crate::physics::{sum_thrusters, PHYSICS_DT};
use crate::world::{World, WORLD_HEIGHT, WORLD_WIDTH, TERRAIN_MAX_X, TERRAIN_MIN_X, VIEW_WIDTH};

const SPAWN_ALTITUDE: f32 = 50.0;
const MIN_SPAWN_Y: f32 = 12.0;
const SPAWN_DOWNRANGE: f32 = 520.0;
const INITIAL_ORBITAL_SPEED: f32 = 18.0;
const INITIAL_DESCENT_VY: f32 = 2.5;
const ORBITAL_STEER_RATE: f32 = 1.2;
const MAX_LANDING_VY: f32 = 3.0;
const MAX_LANDING_VX: f32 = 2.0;
const MAX_LANDING_ANGLE: f32 = 15.0_f32.to_radians();
/// Perfect soft landing with full tanks.
const MAX_LANDING_SCORE: f32 = 10_000.0;
const FUEL_SCORE_WEIGHT: f32 = 0.5;
const SOFT_SCORE_WEIGHT: f32 = 0.5;
/// Avoid startup hitches (shader compile, window init) simulating many physics steps in one frame.
const MAX_FRAME_DT: f32 = 0.05;
const MAX_PHYSICS_STEPS_PER_FRAME: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    Flying,
    Landed,
    Crashed,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct LandingScore {
    pub total: u32,
    pub fuel: u32,
    pub soft: u32,
}

#[derive(Resource)]
pub struct GameState {
    pub lander: Lander,
    pub world: World,
    pub status: GameStatus,
    pub fuel: f32,
    /// Set on a successful landing; zero while flying or after a crash.
    pub score: LandingScore,
    pub seed: u64,
    pub approach_altitude: f32,
    pub initial_orbital_speed: f32,
    pub approach_vx_sign: f32,
    pub autopilot: bool,
    autopilot_smoothed_pitch: f32,
    accumulator: f32,
}

impl GameState {
    pub fn new(seed: u64) -> Self {
        let world = World::generate(seed);
        let spawn_y = (world.pad_y - SPAWN_ALTITUDE).max(MIN_SPAWN_Y);
        let spawn_x = (world.pad_center_x + SPAWN_DOWNRANGE).clamp(8.0, WORLD_WIDTH - 8.0);
        let spawn = Vec2::new(spawn_x, spawn_y);

        let approach_vx_sign = -1.0;
        let initial_vel =
            Vec2::new(approach_vx_sign * INITIAL_ORBITAL_SPEED, INITIAL_DESCENT_VY);

        let mut lander = Lander::new(spawn);
        lander.body.vel = initial_vel;
        lander.body.set_approach_attitude(initial_vel);
        lander.update_hull_world();

        let approach_altitude = world.altitude(spawn.x, spawn.y);

        Self {
            lander,
            world,
            status: GameStatus::Flying,
            fuel: FUEL_CAPACITY,
            score: LandingScore::default(),
            seed,
            approach_altitude,
            initial_orbital_speed: INITIAL_ORBITAL_SPEED,
            approach_vx_sign,
            autopilot: false,
            autopilot_smoothed_pitch: 0.0,
            accumulator: 0.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.seed);
    }

    pub fn new_level(&mut self) {
        self.seed = self.seed.wrapping_add(1);
        *self = Self::new(self.seed);
    }

    pub fn simulate_autopilot_until<F>(&mut self, mut stop: F)
    where
        F: FnMut(&Self) -> bool,
    {
        self.autopilot = true;
        self.autopilot_smoothed_pitch = self.lander.body.angle;

        let input = ThrustInput::default();
        for _ in 0..20_000 {
            if self.status != GameStatus::Flying {
                break;
            }
            if stop(self) {
                break;
            }
            physics_step(self, &input);
        }
    }

    pub fn landed_demo(seed: u64) -> Self {
        let mut game = Self::new(seed);
        game.status = GameStatus::Landed;
        game.lander.body.pos.x = game.world.pad_center_x;
        game.lander.body.pos.y = game.world.pad_y - 1.6;
        game.lander.body.vel = Vec2::ZERO;
        game.lander.body.angle = 0.0;
        game.lander.body.angular_vel = 0.0;
        game.lander.set_throttles(0.0, 0.0, 0.0);
        game.lander.update_hull_world();
        let penetration = game.world.pad_y - game.lander.lowest_point();
        game.lander.body.pos.y += penetration;
        game.lander.update_hull_world();
        game.fuel = 1_640.0;
        game.score = compute_landing_score(game.fuel, 0.0, 0.0, 0.0);
        game
    }

    pub fn camera_offset(&self) -> Vec2 {
        let view_width_m = VIEW_WIDTH;
        let view_height_m = crate::world::SCREEN_HEIGHT / crate::physics::PIXELS_PER_METER;
        let lander_x = self.lander.body.pos.x;
        let lander_y = self.lander.body.pos.y;

        let mut cam_x = lander_x - view_width_m * 0.5;
        let min_cam_x = TERRAIN_MIN_X;
        let max_cam_x = (TERRAIN_MAX_X - view_width_m).max(min_cam_x);
        cam_x = cam_x.clamp(min_cam_x, max_cam_x);

        let mut cam_y = lander_y - view_height_m * 0.35;
        let max_y = (self.world.pad_y - view_height_m * 0.65).max(0.0);
        cam_y = cam_y.min(max_y);
        let min_y = lander_y - view_height_m * 0.85;
        cam_y = cam_y.max(min_y);

        Vec2::new(cam_x, cam_y)
    }
}

pub fn update_game(
    time: Res<Time>,
    mut input: ResMut<ThrustInput>,
    mut game: ResMut<GameState>,
) {
    if input.reset {
        game.reset();
        input.toggle_autopilot = false;
        input.reset = false;
        input.new_level = false;
        return;
    }
    if input.new_level {
        game.new_level();
        input.toggle_autopilot = false;
        input.reset = false;
        input.new_level = false;
        return;
    }

    if input.toggle_autopilot {
        game.autopilot = !game.autopilot;
        if game.autopilot {
            game.autopilot_smoothed_pitch = game.lander.body.angle;
        }
    }

    if game.status != GameStatus::Flying {
        input.toggle_autopilot = false;
        return;
    }

    let dt = time.delta_secs().min(MAX_FRAME_DT);
    game.accumulator += dt;
    let mut steps = 0;
    while game.accumulator >= PHYSICS_DT && steps < MAX_PHYSICS_STEPS_PER_FRAME {
        physics_step(&mut game, &input);
        game.accumulator -= PHYSICS_DT;
        steps += 1;
    }

    input.toggle_autopilot = false;
}

fn physics_step(game: &mut GameState, input: &ThrustInput) {
    let (mut main, mut rot_l, mut rot_r) = if game.autopilot {
        autopilot::compute_thrust(
            &game.lander,
            &game.world,
            &mut game.autopilot_smoothed_pitch,
        )
    } else {
        thrust_from_input(input)
    };

    let thrust_active = main + rot_l + rot_r;
    if thrust_active > 0.0 && game.fuel > 0.0 {
        let burn = FUEL_BURN_RATE * thrust_active.min(1.0) * PHYSICS_DT;
        game.fuel = (game.fuel - burn).max(0.0);
    } else {
        main = 0.0;
        rot_l = 0.0;
        rot_r = 0.0;
    }

    if game.fuel <= 0.0 {
        main = 0.0;
        rot_l = 0.0;
        rot_r = 0.0;
    }

    game.lander.set_throttles(main, rot_l, rot_r);

    let thrusters = game.lander.active_thrusters(main, rot_l, rot_r);
    let (force, torque) = sum_thrusters(&game.lander.body, &thrusters);

    game.lander.body.apply_force_and_torque(force, torque, PHYSICS_DT);
    game.lander.body.apply_gravity(PHYSICS_DT);
    if !game.autopilot {
        apply_orbital_descent(game);
    }
    game.lander.body.integrate(PHYSICS_DT);
    game.lander.update_hull_world();

    if game.world.check_collision(&game.lander.hull_world) {
        resolve_landing(game);
    }

    let cam = game.camera_offset();
    if game.lander.body.pos.y > WORLD_HEIGHT + 10.0
        || flew_off_horizontal_view(game.lander.body.pos.x, cam.x)
    {
        game.status = GameStatus::Crashed;
        game.score = LandingScore::default();
    }
}

fn flew_off_horizontal_view(lander_x: f32, cam_x: f32) -> bool {
    const MARGIN: f32 = 3.0;
    let view_left = cam_x;
    let view_right = cam_x + VIEW_WIDTH;
    lander_x < view_left - MARGIN || lander_x > view_right + MARGIN
}

fn apply_orbital_descent(game: &mut GameState) {
    let pos = game.lander.body.pos;
    let alt = game
        .world
        .altitude(pos.x, pos.y)
        .min(game.approach_altitude);

    let alt_ratio = if game.approach_altitude > 0.0 {
        (alt / game.approach_altitude).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let target_vx = game.approach_vx_sign * game.initial_orbital_speed * alt_ratio.sqrt();
    let steer = (ORBITAL_STEER_RATE * PHYSICS_DT).min(1.0);
    let vx = game.lander.body.vel.x;
    game.lander.body.vel.x += (target_vx - vx) * steer;
}

fn resolve_landing(game: &mut GameState) {
    let com = game.lander.body.pos;
    let on_pad = game.world.is_on_pad(com.x);
    let vy = game.lander.body.vel.y.abs();
    let vx = game.lander.body.vel.x.abs();
    let angle = game.lander.body.angle.abs();

    if on_pad && vy <= MAX_LANDING_VY && vx <= MAX_LANDING_VX && angle <= MAX_LANDING_ANGLE {
        game.status = GameStatus::Landed;
        game.score = compute_landing_score(game.fuel, vy, vx, angle);
        game.lander.body.vel = Vec2::ZERO;
        game.lander.body.angular_vel = 0.0;
        game.lander.set_throttles(0.0, 0.0, 0.0);
        let penetration = game.world.pad_y - game.lander.lowest_point();
        game.lander.body.pos.y += penetration;
        game.lander.update_hull_world();
    } else {
        game.status = GameStatus::Crashed;
        game.score = LandingScore::default();
        game.lander.set_throttles(0.0, 0.0, 0.0);
    }
}

/// Landing softness in \[0, 1\]: 1 is a dead-stop, 0 is at the crash limits.
fn landing_softness(vy: f32, vx: f32, angle: f32) -> f32 {
    let soft_vy = 1.0 - (vy / MAX_LANDING_VY).clamp(0.0, 1.0);
    let soft_vx = 1.0 - (vx / MAX_LANDING_VX).clamp(0.0, 1.0);
    let soft_angle = 1.0 - (angle / MAX_LANDING_ANGLE).clamp(0.0, 1.0);
    soft_vy * 0.55 + soft_vx * 0.25 + soft_angle * 0.20
}

/// Score from remaining fuel and how soft the touchdown was (max [`MAX_LANDING_SCORE`]).
pub fn compute_landing_score(fuel: f32, vy: f32, vx: f32, angle: f32) -> LandingScore {
    let fuel_frac = (fuel / FUEL_CAPACITY).clamp(0.0, 1.0);
    let softness = landing_softness(vy, vx, angle);
    // Square softness so near-limit landings earn little on the soft axis.
    let soft_frac = softness * softness;
    let fuel_score = (MAX_LANDING_SCORE * FUEL_SCORE_WEIGHT * fuel_frac).round() as u32;
    let soft_score = (MAX_LANDING_SCORE * SOFT_SCORE_WEIGHT * soft_frac).round() as u32;
    LandingScore {
        total: fuel_score + soft_score,
        fuel: fuel_score,
        soft: soft_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_landing_with_full_fuel_is_max_score() {
        let score = compute_landing_score(FUEL_CAPACITY, 0.0, 0.0, 0.0);
        assert_eq!(score.total, MAX_LANDING_SCORE as u32);
        assert_eq!(score.fuel, (MAX_LANDING_SCORE * FUEL_SCORE_WEIGHT) as u32);
        assert_eq!(score.soft, (MAX_LANDING_SCORE * SOFT_SCORE_WEIGHT) as u32);
    }

    #[test]
    fn empty_tanks_still_score_from_softness() {
        let score = compute_landing_score(0.0, 0.0, 0.0, 0.0);
        assert_eq!(score.fuel, 0);
        assert_eq!(score.soft, (MAX_LANDING_SCORE * SOFT_SCORE_WEIGHT) as u32);
        assert_eq!(score.total, score.fuel + score.soft);
    }

    #[test]
    fn hard_landing_scores_less_than_soft() {
        let soft = compute_landing_score(1_000.0, 0.2, 0.1, 0.0);
        let hard = compute_landing_score(1_000.0, MAX_LANDING_VY, MAX_LANDING_VX, MAX_LANDING_ANGLE);
        assert!(soft.total > hard.total);
        assert_eq!(soft.fuel, hard.fuel);
        assert!(soft.soft > hard.soft);
        assert!(hard.total > 0);
    }

    #[test]
    fn more_fuel_scores_higher() {
        let low = compute_landing_score(200.0, 0.5, 0.2, 0.05);
        let high = compute_landing_score(1_800.0, 0.5, 0.2, 0.05);
        assert!(high.total > low.total);
        assert!(high.fuel > low.fuel);
        assert_eq!(high.soft, low.soft);
    }
}
