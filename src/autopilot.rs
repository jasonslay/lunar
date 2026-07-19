use glam::Vec2;

use crate::lander::{Lander, LANDER_MASS, MAIN_THRUST_FORCE};
use crate::physics::MOON_GRAVITY;
use crate::world::World;

const KP_VY: f32 = 0.9;
const KP_ANGLE: f32 = 2.6;
const KD_ANGLE: f32 = 2.8;
const KP_PITCH: f32 = 0.05;
const PITCH_SMOOTH: f32 = 0.25;
const ROT_DEADBAND: f32 = 0.03;
const ROT_MAX: f32 = 0.85;
const MAX_GUIDANCE_PITCH: f32 = 0.65;
const HIGH_ALT_MAX_PITCH: f32 = 0.8;
const RECOVER_ANGLE: f32 = 0.9;
const FLIP_ANGLE: f32 = std::f32::consts::FRAC_PI_2 - 0.05;
const HOVER_THROTTLE: f32 = LANDER_MASS * MOON_GRAVITY / MAIN_THRUST_FORCE;

fn normalize_angle(angle: f32) -> f32 {
    let mut a = angle;
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

fn attitude_thrust(lander: &Lander, target_angle: f32, gain_p: f32, gain_d: f32) -> (f32, f32) {
    let angle_err = normalize_angle(target_angle - lander.body.angle);
    if angle_err.abs() < ROT_DEADBAND && lander.body.angular_vel.abs() < 0.12 {
        return (0.0, 0.0);
    }

    let rot_cmd = angle_err * gain_p - lander.body.angular_vel * gain_d;

    let tilt_left = if rot_cmd < 0.0 {
        (-rot_cmd).clamp(0.0, ROT_MAX)
    } else {
        0.0
    };
    let tilt_right = if rot_cmd > 0.0 {
        rot_cmd.clamp(0.0, ROT_MAX)
    } else {
        0.0
    };

    (tilt_left, tilt_right)
}

fn upward_thrust_fraction(main_dir: Vec2) -> f32 {
    (-main_dir.y).clamp(0.0, 1.0)
}

fn main_throttle_for_vertical(
    lander: &Lander,
    desired_vertical_accel: f32,
    max_throttle: f32,
) -> f32 {
    let main_dir = lander.world_thrust_direction(&lander.main_thruster);
    let upward = upward_thrust_fraction(main_dir);
    if upward < 0.15 {
        return 0.0;
    }

    // Net accel (+y = down): -throttle*MAX*upward/mass + G = desired
    let thrust_needed = LANDER_MASS * (MOON_GRAVITY - desired_vertical_accel);
    (thrust_needed / (MAIN_THRUST_FORCE * upward))
        .clamp(0.0, max_throttle)
}

fn recovery_thrust(lander: &Lander) -> (f32, f32, f32) {
    let (tilt_left, tilt_right) = attitude_thrust(lander, 0.0, 3.0, 3.0);

    let main_dir = lander.world_thrust_direction(&lander.main_thruster);
    let upward = upward_thrust_fraction(main_dir);
    let mut main = 0.0;

    if upward > 0.55 && lander.body.angle.abs() < 1.05 {
        main = main_throttle_for_vertical(lander, -0.3, 0.65);
    }

    (main, tilt_left, tilt_right)
}

fn in_landing_corridor(alt: f32, range: f32, pad_half: f32) -> bool {
    alt < 22.0 && range < pad_half + 6.0
}

fn target_horizontal_velocity(dx: f32, alt: f32, vx: f32, pad_half: f32) -> f32 {
    let toward_pad = dx.signum();
    let range = dx.abs();

    // Over the pad: steer toward center and bleed off lateral speed.
    if in_landing_corridor(alt, range, pad_half) {
        let edge = (range / pad_half).clamp(0.0, 1.25);
        let alt_norm = (1.0 - (alt / 22.0).clamp(0.0, 1.0)).powf(0.55);
        let pos_gain = 0.42 + edge * 0.38 + alt_norm * 0.22;
        let vel_gain = 0.62 + edge * 0.18;
        let cap = 0.95 + edge * 0.55 + alt_norm * 0.25;
        if range < 0.35 && alt < 2.0 && vx.abs() < 0.15 {
            return 0.0;
        }
        return (dx * pos_gain - vx * vel_gain).clamp(-cap, cap);
    }

    // Still outside the pad lip — start closing before stepping onto the edge markers.
    if alt < 20.0 && range >= pad_half && range < pad_half + 14.0 {
        return (dx * 0.36 - vx * 0.54).clamp(-1.15, 1.15);
    }

    if range < 1.0 && alt < 4.0 && vx.abs() < 0.3 {
        return 0.0;
    }

    if range < 2.5 {
        return 0.0;
    }

    let brake_accel = if range < 25.0 { 2.0 } else { 1.1 };
    let max_speed = if range < 20.0 {
        1.0
    } else if range < 50.0 {
        2.5
    } else if alt > 30.0 {
        15.0
    } else {
        5.0
    };

    let stopping_speed = (2.0 * brake_accel * range).sqrt();

    if alt > 30.0 && range > 80.0 {
        let descent_seconds = (alt / 1.6).clamp(5.0, 40.0);
        let glide_speed = (range / descent_seconds).min(15.0);
        return toward_pad * glide_speed.min(stopping_speed).min(max_speed);
    }

    toward_pad * stopping_speed.min(max_speed)
}

fn target_vertical_velocity(alt: f32, range: f32, _pad_half: f32) -> f32 {
    if range > 80.0 {
        if alt < 12.0 {
            -0.5
        } else if alt < 22.0 {
            -0.15
        } else {
            (alt * 0.025).clamp(0.35, 1.0)
        }
    } else if range > 40.0 {
        if alt > 10.0 { 0.6 } else { 0.25 }
    } else if range > 15.0 {
        if alt > 6.0 { 1.0 } else { 0.4 }
    } else if alt > 40.0 {
        1.0
    } else if alt > 6.0 {
        (alt * 0.35).sqrt().min(2.4)
    } else if alt > 1.5 {
        (alt * 0.2).max(0.15)
    } else {
        0.0
    }
}

pub fn compute_thrust(
    lander: &Lander,
    world: &World,
    smoothed_pitch: &mut f32,
) -> (f32, f32, f32) {
    let angle = lander.body.angle;
    if angle.abs() > RECOVER_ANGLE || angle.abs() > FLIP_ANGLE {
        return recovery_thrust(lander);
    }

    let pos = lander.body.pos;
    let vel = lander.body.vel;
    let alt = world.altitude(pos.x, pos.y).max(0.1);
    let dx = world.pad_center_x - pos.x;
    let range = dx.abs();
    let pad_half = (world.pad_end_x - world.pad_start_x) * 0.5;

    let target_vx = target_horizontal_velocity(dx, alt, vel.x, pad_half);
    let target_vy = target_vertical_velocity(alt, dx.abs(), pad_half);
    let vx_err = target_vx - vel.x;
    let vy_err = target_vy - vel.y;

    let mut max_pitch = if alt > 25.0 {
        HIGH_ALT_MAX_PITCH
    } else {
        MAX_GUIDANCE_PITCH
    };
    if in_landing_corridor(alt, range, pad_half) {
        let edge = (range / pad_half).clamp(0.0, 1.0);
        max_pitch = max_pitch.max(0.5 + edge * 0.18);
    }

    let mut pitch_gain = KP_PITCH;
    if range < 200.0 && vel.x.abs() > 2.0 {
        pitch_gain = 0.07 + (200.0 - range.min(200.0)) / 200.0 * 0.05;
    }
    if alt < 35.0 && vel.x.abs() > 3.0 {
        pitch_gain = pitch_gain.max(0.08);
    }
    if alt < 12.0 && vel.x.abs() > 1.0 {
        pitch_gain = pitch_gain.max(0.1);
    }
    if alt < 8.0 && vel.x.abs() > 0.8 {
        pitch_gain = pitch_gain.max(0.12);
    }
    if in_landing_corridor(alt, range, pad_half) {
        let edge = (range / pad_half).clamp(0.0, 1.0);
        pitch_gain = pitch_gain.max(0.12 + edge * 0.08);
    }
    // Moving away from pad center — brake back toward the middle.
    if range > 0.5 && dx.signum() != vel.x.signum() && vel.x.abs() > 0.25 {
        pitch_gain = pitch_gain.max(0.14);
    }

    let raw_target = (vx_err * pitch_gain).clamp(-max_pitch, max_pitch);
    let pitch_smooth = if range < 120.0
        || vx_err.abs() > 2.5
        || vel.x.abs() > 4.0
        || (range > 80.0 && alt > 35.0)
        || in_landing_corridor(alt, range, pad_half)
    {
        1.0
    } else {
        PITCH_SMOOTH
    };
    *smoothed_pitch += (raw_target - *smoothed_pitch) * pitch_smooth;
    let mut target_angle = *smoothed_pitch;

    if alt < 7.0 && dx.abs() < pad_half + 5.0 && dx.abs() < 0.4 {
        let blend = (1.0 - alt / 7.0).clamp(0.0, 1.0);
        target_angle *= 1.0 - blend;
    }

    let (tilt_left, tilt_right) = attitude_thrust(lander, target_angle, KP_ANGLE, KD_ANGLE);

    let desired_vertical_accel = vy_err * KP_VY;
    let main_dir = lander.world_thrust_direction(&lander.main_thruster);
    let upward = upward_thrust_fraction(main_dir);
    let mut main = main_throttle_for_vertical(lander, desired_vertical_accel, 1.0);

    // Pitched braking steals vertical thrust — add modest compensation only when descending too fast.
    if range > 40.0 && alt > 12.0 && upward < 0.85 && vel.y > target_vy + 1.5 {
        let hover = (HOVER_THROTTLE / upward).min(0.75);
        main = main.max(hover);
    }

    if alt < 20.0 && vel.y > 1.2 && main < HOVER_THROTTLE {
        main = main.max(HOVER_THROTTLE.min(0.85));
    }

    (main, tilt_left, tilt_right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameState, GameStatus};
    use crate::physics::{sum_thrusters, PHYSICS_DT};
    use crate::world::{World, WORLD_MIN_X, WORLD_WIDTH};

    fn autopilot_physics_step(lander: &mut Lander, world: &World, smoothed_pitch: &mut f32) {
        let (main, tilt_left, tilt_right) = compute_thrust(lander, world, smoothed_pitch);
        lander.set_throttles(main, tilt_left, tilt_right);

        let thrusters = lander.active_thrusters(main, tilt_left, tilt_right);
        let (force, torque) = sum_thrusters(&lander.body, &thrusters);
        lander.body.apply_force_and_torque(force, torque, PHYSICS_DT);
        lander.body.apply_gravity(PHYSICS_DT);
        lander.body.integrate(PHYSICS_DT);
        lander.update_hull_world();
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SimOutcome {
        Flying,
        Landed,
        Crashed,
    }

    #[derive(Debug)]
    struct SimState {
        outcome: SimOutcome,
        steps: usize,
        pos: Vec2,
        vel: Vec2,
        angle: f32,
    }

    fn simulate_autopilot(world: &World, lander: &mut Lander, max_steps: usize) -> SimState {
        const MAX_LANDING_VY: f32 = 3.0;
        const MAX_LANDING_VX: f32 = 2.0;
        const MAX_LANDING_ANGLE: f32 = 15.0_f32.to_radians();

        let mut smoothed_pitch = lander.body.angle;

        for step in 0..max_steps {
            autopilot_physics_step(lander, world, &mut smoothed_pitch);

            if world.check_collision(&lander.hull_world) {
                let on_pad = world.is_on_pad(lander.body.pos.x);
                let vy = lander.body.vel.y.abs();
                let vx = lander.body.vel.x.abs();
                let angle = lander.body.angle.abs();

                let outcome = if on_pad
                    && vy <= MAX_LANDING_VY
                    && vx <= MAX_LANDING_VX
                    && angle <= MAX_LANDING_ANGLE
                {
                    SimOutcome::Landed
                } else {
                    SimOutcome::Crashed
                };

                return SimState {
                    outcome,
                    steps: step + 1,
                    pos: lander.body.pos,
                    vel: lander.body.vel,
                    angle: lander.body.angle,
                };
            }
        }

        SimState {
            outcome: SimOutcome::Flying,
            steps: max_steps,
            pos: lander.body.pos,
            vel: lander.body.vel,
            angle: lander.body.angle,
        }
    }

    fn spawn_approach_lander(world: &World) -> Lander {
        let spawn_y = (world.pad_y - 50.0).max(12.0);
        let spawn_x = (world.pad_center_x + 480.0).clamp(8.0, WORLD_WIDTH - 8.0);
        let initial_vel = Vec2::new(-18.0, 2.5);

        let mut lander = Lander::new(Vec2::new(spawn_x, spawn_y));
        lander.body.vel = initial_vel;
        lander.body.set_approach_attitude(initial_vel);
        lander.update_hull_world();
        lander
    }

    #[test]
    fn aims_left_when_spawned_downrange() {
        let world = World::generate(42);
        let lander = spawn_approach_lander(&world);
        let dx = world.pad_center_x - lander.body.pos.x;
        let alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
        let target_vx = target_horizontal_velocity(dx, alt, lander.body.vel.x, 8.0);

        assert!(dx < -400.0);
        assert!(target_vx < -10.0, "expected strong leftward closure, got {target_vx}");
    }

    #[test]
    fn zeros_horizontal_target_over_pad() {
        assert_eq!(target_horizontal_velocity(1.0, 25.0, 0.0, 8.0), 0.0);
    }

    #[test]
    fn steers_toward_pad_center_on_final_approach() {
        let vx = target_horizontal_velocity(2.0, 3.0, 0.4, 8.0);
        assert!(vx > 0.5, "expected rightward closure toward center, got {vx}");
    }

    #[test]
    fn steers_from_pad_edge_toward_center() {
        let vx = target_horizontal_velocity(-7.5, 8.0, 1.5, 8.0);
        assert!(vx < -0.6, "expected leftward closure from pad edge, got {vx}");
    }

    #[test]
    fn does_not_flip_during_initial_recovery() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);

        let mut smoothed_pitch = lander.body.angle;
        for _ in 0..600 {
            autopilot_physics_step(&mut lander, &world, &mut smoothed_pitch);
            assert!(
                lander.body.angle.abs() < FLIP_ANGLE + 0.05,
                "tumbled past safe angle: {} rad",
                lander.body.angle
            );
        }
    }

    #[test]
    fn levels_out_after_engaging_autopilot() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);

        let mut smoothed_pitch = lander.body.angle;
        for _ in 0..900 {
            autopilot_physics_step(&mut lander, &world, &mut smoothed_pitch);
        }

        assert!(
            lander.body.angle.abs() < 0.45,
            "expected near-upright attitude, got {} rad",
            lander.body.angle
        );
    }

    #[test]
    fn approach_trajectory_stays_in_bounds() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);
        let mut smoothed_pitch = lander.body.angle;

        let mut max_alt = 0.0f32;
        let mut min_x = f32::INFINITY;

        for _ in 0..14_000 {
            let alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
            max_alt = max_alt.max(alt);
            min_x = min_x.min(lander.body.pos.x);

            autopilot_physics_step(&mut lander, &world, &mut smoothed_pitch);
            if world.check_collision(&lander.hull_world) {
                break;
            }
        }

        assert!(
            max_alt < 90.0,
            "climbed too high: {max_alt} m"
        );
        assert!(
            min_x >= WORLD_MIN_X,
            "overshot left edge: min_x={min_x}"
        );
    }

    #[test]
    fn approach_after_delayed_autopilot_engagement() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);

        // Coast like manual flight before engaging AP.
        for _ in 0..180 {
            lander.body.apply_gravity(PHYSICS_DT);
            lander.body.integrate(PHYSICS_DT);
            lander.update_hull_world();
        }

        let mut smoothed_pitch = lander.body.angle;
        let mut max_alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
        let mut min_x = lander.body.pos.x;

        for _ in 0..14_000 {
            let alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
            max_alt = max_alt.max(alt);
            min_x = min_x.min(lander.body.pos.x);
            autopilot_physics_step(&mut lander, &world, &mut smoothed_pitch);
            if world.check_collision(&lander.hull_world) {
                break;
            }
        }

        assert!(max_alt < 120.0, "delayed AP climbed too high: {max_alt}");
        assert!(min_x >= WORLD_MIN_X, "delayed AP overshot left: min_x={min_x}");
    }


    #[test]
    fn lands_near_pad_center_from_default_spawn() {
        for seed in [42u64, 1, 7, 99, 12345, 555] {
            let mut game = GameState::new(seed);
            game.simulate_autopilot_until(|_| false);
            assert_eq!(game.status, GameStatus::Landed, "seed {seed}");
            let dx = (game.lander.body.pos.x - game.world.pad_center_x).abs();
            assert!(
                dx < 4.0,
                "seed {seed} landed {dx:.2} m from pad center"
            );
        }
    }

    #[test]
    fn lands_on_pad_from_approach() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);

        let result = simulate_autopilot(&world, &mut lander, 14_000);

        assert_eq!(
            result.outcome,
            SimOutcome::Landed,
            "expected pad landing, got {:?} at pos {:?} vel {:?} angle {} after {} steps",
            result.outcome,
            result.pos,
            result.vel,
            result.angle,
            result.steps,
        );
        assert!(world.is_on_pad(result.pos.x));
        let dx = (result.pos.x - world.pad_center_x).abs();
        assert!(
            dx < 1.6,
            "expected landing near pad center, offset {dx:.2} m"
        );
    }

    #[test]
    fn autopilot_screenshot_pose_is_inflight_near_pad() {
        let mut game = GameState::new(42);
        game.simulate_autopilot_until(|game| {
            let dx = (game.world.pad_center_x - game.lander.body.pos.x).abs();
            let alt = game.world.clearance_above_terrain(&game.lander.hull_world);
            dx < 28.0 && alt < 16.0 && alt > 5.0
        });

        assert_eq!(game.status, GameStatus::Flying);
        assert!(game.autopilot);
        let alt = game.world.clearance_above_terrain(&game.lander.hull_world);
        assert!(alt > 5.0 && alt < 16.0, "expected final approach altitude, got {alt}");
        assert!(game.lander.throttle_main > 0.1, "expected main engine firing on approach");
    }

    #[test]
    fn lands_when_starting_above_pad() {
        let world = World::generate(42);
        let mut lander = Lander::new(Vec2::new(world.pad_center_x, world.pad_y - 35.0));
        lander.body.vel = Vec2::new(0.0, 0.4);
        lander.body.angle = 0.0;
        lander.update_hull_world();

        let result = simulate_autopilot(&world, &mut lander, 6_000);

        assert_eq!(
            result.outcome,
            SimOutcome::Landed,
            "vertical descent failed: {:?} pos {:?} vel {:?}",
            result.outcome,
            result.pos,
            result.vel
        );
    }
}
