use glam::Vec2;

use crate::physics::{RigidBody, Thruster};

pub const LANDER_WIDTH: f32 = 4.5;
pub const LANDER_HEIGHT: f32 = 4.0;
pub const LANDER_MASS: f32 = 16_000.0;
pub const MAIN_THRUST_FORCE: f32 = 45_000.0;
pub const ROTATE_THRUST_FORCE: f32 = 4_500.0;
pub const FUEL_CAPACITY: f32 = 2_000.0;
pub const FUEL_BURN_RATE: f32 = 15.0;

pub struct Lander {
    pub body: RigidBody,
    pub main_thruster: Thruster,
    pub rotate_ccw: Thruster,
    pub rotate_cw: Thruster,
    pub hull_local: Vec<Vec2>,
    pub hull_world: Vec<Vec2>,
    pub frame_local: Vec<(Vec2, Vec2)>,
    pub throttle_main: f32,
    pub throttle_tilt_left: f32,
    pub throttle_tilt_right: f32,
}

impl Lander {
    pub fn new(spawn_pos: Vec2) -> Self {
        let half_w = LANDER_WIDTH / 2.0;

        let cabin_top = Vec2::new(0.0, -2.0);
        let cabin_bl = Vec2::new(-1.0, -0.75);
        let cabin_br = Vec2::new(1.0, -0.75);
        let engine = Vec2::new(0.0, 1.25);
        let leg_l = Vec2::new(-half_w, 1.5);
        let leg_r = Vec2::new(half_w, 1.5);
        let pad_half = 0.5;

        let hull_local = vec![cabin_top, cabin_bl, leg_l, engine, leg_r, cabin_br];

        let frame_local = vec![
            (cabin_top, cabin_br),
            (cabin_br, cabin_bl),
            (cabin_bl, cabin_top),
            (cabin_bl, leg_l),
            (cabin_br, leg_r),
            (engine, cabin_bl),
            (engine, cabin_br),
            (
                Vec2::new(leg_l.x - pad_half, leg_l.y),
                Vec2::new(leg_l.x + pad_half, leg_l.y),
            ),
            (
                Vec2::new(leg_r.x - pad_half, leg_r.y),
                Vec2::new(leg_r.x + pad_half, leg_r.y),
            ),
            (cabin_top, Vec2::new(0.0, -2.5)),
        ];

        Self {
            body: RigidBody::new(spawn_pos, LANDER_MASS, LANDER_WIDTH, LANDER_HEIGHT),
            main_thruster: Thruster {
                local_offset: engine,
                local_direction: Vec2::new(0.0, -1.0),
                max_force: MAIN_THRUST_FORCE,
            },
            rotate_cw: Thruster {
                local_offset: leg_l,
                local_direction: Vec2::new(-0.4, -0.9).normalize(),
                max_force: ROTATE_THRUST_FORCE,
            },
            rotate_ccw: Thruster {
                local_offset: leg_r,
                local_direction: Vec2::new(0.4, -0.9).normalize(),
                max_force: ROTATE_THRUST_FORCE,
            },
            hull_local,
            hull_world: vec![Vec2::ZERO; 6],
            frame_local,
            throttle_main: 0.0,
            throttle_tilt_left: 0.0,
            throttle_tilt_right: 0.0,
        }
    }

    pub fn active_thrusters(&self, main: f32, tilt_left: f32, tilt_right: f32) -> Vec<(Thruster, f32)> {
        vec![
            (self.main_thruster.clone(), main),
            (self.rotate_ccw.clone(), tilt_left),
            (self.rotate_cw.clone(), tilt_right),
        ]
    }

    pub fn set_throttles(&mut self, main: f32, tilt_left: f32, tilt_right: f32) {
        self.throttle_main = main;
        self.throttle_tilt_left = tilt_left;
        self.throttle_tilt_right = tilt_right;
    }

    pub fn world_thrust_direction(&self, thruster: &Thruster) -> Vec2 {
        self.body.rotate_local_to_world(thruster.local_direction.normalize())
    }

    pub fn thruster_world_position(&self, thruster: &Thruster) -> Vec2 {
        self.body.pos + self.body.rotate_local_to_world(thruster.local_offset)
    }

    pub fn update_hull_world(&mut self) {
        for (i, local) in self.hull_local.iter().enumerate() {
            self.hull_world[i] = self.body.pos + self.body.rotate_local_to_world(*local);
        }
    }

    pub fn thruster_world_positions(&self) -> [Vec2; 3] {
        [
            self.body.pos + self.body.rotate_local_to_world(self.main_thruster.local_offset),
            self.body.pos + self.body.rotate_local_to_world(self.rotate_cw.local_offset),
            self.body.pos + self.body.rotate_local_to_world(self.rotate_ccw.local_offset),
        ]
    }

    pub fn lowest_point(&self) -> f32 {
        self.hull_world
            .iter()
            .map(|v| v.y)
            .fold(f32::NEG_INFINITY, f32::max)
    }
}
