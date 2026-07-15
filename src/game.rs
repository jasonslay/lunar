use bevy::prelude::*;
use glam::Vec2;

use crate::autopilot;
use crate::input::{thrust_from_input, ThrustInput};
use crate::lander::{Lander, FUEL_BURN_RATE, FUEL_CAPACITY};
use crate::physics::{sum_thrusters, PHYSICS_DT};
use crate::world::{World, WORLD_HEIGHT, WORLD_WIDTH};

const SPAWN_ALTITUDE: f32 = 50.0;
const MIN_SPAWN_Y: f32 = 12.0;
const SPAWN_DOWNRANGE: f32 = 480.0;
const INITIAL_ORBITAL_SPEED: f32 = 18.0;
const INITIAL_DESCENT_VY: f32 = 2.5;
const ORBITAL_STEER_RATE: f32 = 1.2;
const MAX_LANDING_VY: f32 = 3.0;
const MAX_LANDING_VX: f32 = 2.0;
const MAX_LANDING_ANGLE: f32 = 15.0_f32.to_radians();
/// Avoid startup hitches (shader compile, window init) simulating many physics steps in one frame.
const MAX_FRAME_DT: f32 = 0.05;
const MAX_PHYSICS_STEPS_PER_FRAME: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Flying,
    Landed,
    Crashed,
}

#[derive(Resource)]
pub struct GameState {
    pub lander: Lander,
    pub world: World,
    pub status: GameStatus,
    pub fuel: f32,
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

    pub fn camera_offset(&self) -> Vec2 {
        let view_width_m = crate::world::VIEW_WIDTH;
        let view_height_m = crate::world::SCREEN_HEIGHT / crate::physics::PIXELS_PER_METER;
        let lander_x = self.lander.body.pos.x;
        let lander_y = self.lander.body.pos.y;

        let mut cam_x = lander_x - view_width_m * 0.5;
        cam_x = cam_x.max(0.0);
        let max_cam_x = (WORLD_WIDTH - view_width_m).max(0.0);
        if lander_x <= WORLD_WIDTH - view_width_m * 0.5 {
            cam_x = cam_x.min(max_cam_x);
        }

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
    if input.toggle_autopilot {
        game.autopilot = !game.autopilot;
        if game.autopilot {
            game.autopilot_smoothed_pitch = game.lander.body.angle;
        }
    }

    if game.status != GameStatus::Flying {
        if input.reset {
            game.reset();
        }
        if input.new_level {
            game.new_level();
        }
        input.toggle_autopilot = false;
        input.reset = false;
        input.new_level = false;
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
    input.reset = false;
    input.new_level = false;
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

    if game.lander.body.pos.y > WORLD_HEIGHT + 10.0
        || game.lander.body.pos.x < -10.0
        || game.lander.body.pos.x > WORLD_WIDTH + 10.0
    {
        game.status = GameStatus::Crashed;
    }
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
        game.lander.body.vel = Vec2::ZERO;
        game.lander.body.angular_vel = 0.0;
        game.lander.set_throttles(0.0, 0.0, 0.0);
        let penetration = game.world.pad_y - game.lander.lowest_point();
        game.lander.body.pos.y += penetration;
        game.lander.update_hull_world();
    } else {
        game.status = GameStatus::Crashed;
        game.lander.set_throttles(0.0, 0.0, 0.0);
    }
}
