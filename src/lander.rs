use glam::Vec2;

use crate::physics::{RigidBody, Thruster};

pub const LANDER_WIDTH: f32 = 3.2;
pub const LANDER_HEIGHT: f32 = 3.4;
pub const LANDER_MASS: f32 = 16_000.0;
pub const MAIN_THRUST_FORCE: f32 = 45_000.0;
pub const ROTATE_THRUST_FORCE: f32 = 4_500.0;
pub const FUEL_CAPACITY: f32 = 2_000.0;
pub const FUEL_BURN_RATE: f32 = 15.0;

/// Solid fill for ascent + descent stages (legs drawn as wireframe only).
pub const LANDER_BODY_FILL: [Vec2; 9] = [
    Vec2::new(0.0, -1.76),
    Vec2::new(-0.44, -1.31),
    Vec2::new(-0.54, -0.38),
    Vec2::new(-0.99, -0.09),
    Vec2::new(-1.07, 0.66),
    Vec2::new(1.07, 0.66),
    Vec2::new(0.99, -0.09),
    Vec2::new(0.54, -0.38),
    Vec2::new(0.44, -1.31),
];

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
        // Apollo LM side profile (+y down), scaled to match prior gameplay footprint.
        let apex = Vec2::new(0.0, -1.76);
        let probe_tip = Vec2::new(0.0, -2.04);
        let asc_tl = Vec2::new(-0.44, -1.31);
        let asc_tr = Vec2::new(0.44, -1.31);
        let asc_bl = Vec2::new(-0.54, -0.38);
        let asc_br = Vec2::new(0.54, -0.38);

        let des_tl = Vec2::new(-0.99, -0.09);
        let des_tr = Vec2::new(0.99, -0.09);
        let des_bl = Vec2::new(-1.07, 0.66);
        let des_br = Vec2::new(1.07, 0.66);

        let leg_l_knee = Vec2::new(-1.37, 1.14);
        let leg_l_out = Vec2::new(-1.56, 1.56);
        let leg_l_in = Vec2::new(-1.16, 1.56);

        let leg_r_knee = Vec2::new(1.37, 1.14);
        let leg_r_out = Vec2::new(1.56, 1.56);
        let leg_r_in = Vec2::new(1.16, 1.56);

        let engine = Vec2::new(0.0, 1.04);
        let bell_l = Vec2::new(-0.22, 0.84);
        let bell_r = Vec2::new(0.22, 0.84);
        let bell_tip = Vec2::new(0.0, 1.24);

        let rcs_l = Vec2::new(-0.32, -1.62);
        let rcs_r = Vec2::new(0.32, -1.62);

        let hull_local = vec![
            apex,
            asc_tl,
            asc_bl,
            des_tl,
            des_bl,
            leg_l_out,
            leg_l_in,
            bell_tip,
            leg_r_in,
            leg_r_out,
            des_br,
            des_tr,
            asc_br,
            asc_tr,
        ];

        let frame_local = vec![
            // Ascent stage
            (apex, asc_tl),
            (asc_tl, asc_bl),
            (asc_bl, asc_br),
            (asc_br, asc_tr),
            (asc_tr, apex),
            // Interstage / descent deck
            (asc_bl, des_tl),
            (asc_br, des_tr),
            (des_tl, des_bl),
            (des_bl, des_br),
            (des_br, des_tr),
            (des_tl, des_tr),
            // Descent stage mid-bay
            (Vec2::new(-1.03, 0.28), Vec2::new(1.03, 0.28)),
            // Landing gear — left
            (des_bl, leg_l_knee),
            (leg_l_knee, leg_l_out),
            (leg_l_out, leg_l_in),
            // Landing gear — right
            (des_br, leg_r_knee),
            (leg_r_knee, leg_r_out),
            (leg_r_out, leg_r_in),
            // Engine bell
            (bell_l, bell_tip),
            (bell_tip, bell_r),
            (bell_l, bell_r),
            (des_bl, bell_l),
            (des_br, bell_r),
            // Docking probe
            (apex, probe_tip),
            // RCS clusters at the ascent tip
            (rcs_l, rcs_l + Vec2::new(-0.12, -0.06)),
            (rcs_l, rcs_l + Vec2::new(-0.04, -0.11)),
            (rcs_r, rcs_r + Vec2::new(0.12, -0.06)),
            (rcs_r, rcs_r + Vec2::new(0.04, -0.11)),
        ];

        let hull_len = hull_local.len();

        Self {
            body: RigidBody::new(spawn_pos, LANDER_MASS, LANDER_WIDTH, LANDER_HEIGHT),
            main_thruster: Thruster {
                local_offset: engine,
                local_direction: Vec2::new(0.0, -1.0),
                max_force: MAIN_THRUST_FORCE,
            },
            rotate_cw: Thruster {
                local_offset: rcs_l,
                local_direction: Vec2::new(1.0, -0.12).normalize(),
                max_force: ROTATE_THRUST_FORCE,
            },
            rotate_ccw: Thruster {
                local_offset: rcs_r,
                local_direction: Vec2::new(-1.0, -0.12).normalize(),
                max_force: ROTATE_THRUST_FORCE,
            },
            hull_local,
            hull_world: vec![Vec2::ZERO; hull_len],
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
