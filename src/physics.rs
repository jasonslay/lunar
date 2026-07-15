use glam::Vec2;

pub const PHYSICS_DT: f32 = 1.0 / 60.0;

/// Lunar surface gravity (m/s²).
pub const MOON_GRAVITY: f32 = 1.62;

/// Simulation uses SI units (meters); multiply by this for screen pixels.
pub const PIXELS_PER_METER: f32 = 8.0;

#[derive(Clone, Debug)]
pub struct RigidBody {
    pub pos: Vec2,
    pub vel: Vec2,
    pub angle: f32,
    pub angular_vel: f32,
    pub mass: f32,
    pub inertia: f32,
}

impl RigidBody {
    pub fn new(pos: Vec2, mass: f32, width: f32, height: f32) -> Self {
        let inertia = mass * (width * width + height * height) / 12.0;
        Self {
            pos,
            vel: Vec2::ZERO,
            angle: 0.0,
            angular_vel: 0.0,
            mass,
            inertia,
        }
    }

    pub fn apply_force_and_torque(&mut self, force: Vec2, torque: f32, dt: f32) {
        let linear_accel = force / self.mass;
        let angular_accel = torque / self.inertia;

        self.vel += linear_accel * dt;
        self.angular_vel += angular_accel * dt;
    }

    pub fn apply_gravity(&mut self, dt: f32) {
        self.vel.y += MOON_GRAVITY * dt;
    }

    pub fn integrate(&mut self, dt: f32) {
        self.pos += self.vel * dt;
        self.angle += self.angular_vel * dt;
    }

    pub fn rotate_local_to_world(&self, local: Vec2) -> Vec2 {
        let (sin, cos) = self.angle.sin_cos();
        Vec2::new(
            local.x * cos - local.y * sin,
            local.x * sin + local.y * cos,
        )
    }

    /// Approach attitude: mostly upright with modest pitch into the flight path for braking.
    pub fn set_approach_attitude(&mut self, velocity: Vec2) {
        const MAX_APPROACH_PITCH: f32 = 0.35;

        if velocity.length_squared() > f32::EPSILON {
            let retrograde = (-velocity.x).atan2(velocity.y);
            self.angle = retrograde.clamp(-MAX_APPROACH_PITCH, MAX_APPROACH_PITCH);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Thruster {
    pub local_offset: Vec2,
    pub local_direction: Vec2,
    pub max_force: f32,
}

impl Thruster {
    pub fn contribution(&self, body: &RigidBody, throttle: f32) -> (Vec2, f32) {
        if throttle <= 0.0 {
            return (Vec2::ZERO, 0.0);
        }

        let world_dir = body.rotate_local_to_world(self.local_direction.normalize());
        let force = world_dir * (self.max_force * throttle);
        let r = body.rotate_local_to_world(self.local_offset);
        let torque = r.x * force.y - r.y * force.x;
        (force, torque)
    }
}

pub fn sum_thrusters(body: &RigidBody, thrusters: &[(Thruster, f32)]) -> (Vec2, f32) {
    let mut total_force = Vec2::ZERO;
    let mut total_torque = 0.0;

    for (thruster, throttle) in thrusters {
        let (force, torque) = thruster.contribution(body, *throttle);
        total_force += force;
        total_torque += torque;
    }

    (total_force, total_torque)
}
