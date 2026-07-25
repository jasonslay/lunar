use glam::Vec2;

use crate::lander::{Lander, LANDER_MASS, MAIN_THRUST_FORCE};
use crate::physics::MOON_GRAVITY;
use crate::world::World;

const KP_VY: f32 = 1.05;
const KP_ANGLE: f32 = 2.8;
const KD_ANGLE: f32 = 3.0;
const KP_PITCH: f32 = 0.055;
const PITCH_SMOOTH: f32 = 0.28;
const ROT_DEADBAND: f32 = 0.03;
const ROT_MAX: f32 = 0.85;
const MAX_GUIDANCE_PITCH: f32 = 0.7;
const HIGH_ALT_MAX_PITCH: f32 = 0.85;
const RECOVER_ANGLE: f32 = 0.9;
const FLIP_ANGLE: f32 = std::f32::consts::FRAC_PI_2 - 0.05;
const HOVER_THROTTLE: f32 = LANDER_MASS * MOON_GRAVITY / MAIN_THRUST_FORCE;

/// Horizontal offset (m) inside which final descent is allowed.
const CENTER_CAPTURE_RANGE: f32 = 2.1;
/// Lateral speed (m/s) that must be bled off before committing to touchdown.
const CENTER_CAPTURE_VX: f32 = 0.65;

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
    (thrust_needed / (MAIN_THRUST_FORCE * upward)).clamp(0.0, max_throttle)
}

fn recovery_thrust(lander: &Lander) -> (f32, f32, f32) {
    let (tilt_left, tilt_right) = attitude_thrust(lander, 0.0, 3.2, 3.2);

    let main_dir = lander.world_thrust_direction(&lander.main_thruster);
    let upward = upward_thrust_fraction(main_dir);
    let mut main = 0.0;

    if upward > 0.45 && lander.body.angle.abs() < 1.15 {
        // Prefer climbing out of a tip rather than a soft hover — delayed engages
        // often arrive already falling fast.
        let climb = if lander.body.vel.y > 2.0 { -1.2 } else { -0.45 };
        main = main_throttle_for_vertical(lander, climb, 1.0);
    }

    (main, tilt_left, tilt_right)
}

fn in_landing_corridor(alt: f32, range: f32, pad_half: f32) -> bool {
    alt < 26.0 && range < pad_half + 10.0
}

/// True once the ship is centered and slow enough to commit to soft touchdown.
/// Attitude is leveled separately once this captures — gating on angle caused perpetual hovers.
fn ready_to_land(range: f32, vx: f32) -> bool {
    range < CENTER_CAPTURE_RANGE && vx.abs() < CENTER_CAPTURE_VX
}

fn approach_funnel(dx: f32, alt: f32, vx: f32, pad_half: f32) -> Option<f32> {
    let range = dx.abs();
    if alt >= 20.0 || range < pad_half - 1.0 || range >= pad_half + 18.0 {
        return None;
    }

    let depth = 1.0 - ((range - pad_half + 1.0) / 19.0).clamp(0.0, 1.0);
    let alt_weight = (1.0 - (alt / 20.0).clamp(0.0, 1.0)).powf(0.65);
    let urgency = (depth * 0.7 + alt_weight * 0.3).clamp(0.0, 1.0);
    let pos_gain = 0.4 + urgency * 0.65;
    let vel_gain = 0.5 + urgency * 0.4;
    let cap = 1.1 + urgency * 0.9;
    let target = dx * pos_gain - vx * vel_gain;
    let room = (range - pad_half * 0.2).max(0.35);
    let max_closing = (2.0 * 1.15 * room).sqrt();
    Some(target.clamp(-max_closing.min(cap), max_closing.min(cap)))
}

fn target_horizontal_velocity(dx: f32, alt: f32, vx: f32, pad_half: f32) -> f32 {
    let toward_pad = dx.signum();
    let range = dx.abs();

    // Over / near the pad: overdamped slide to center (no hover-hunt).
    if in_landing_corridor(alt, range, pad_half) {
        if range < 0.45 && vx.abs() < 0.2 {
            return 0.0;
        }

        let closing = dx.signum() as f32 * vx.signum() as f32 > 0.0;
        // Only scrub speed when we'd overshoot the bullseye — don't stop short.
        let stop_speed = (2.0 * 0.55 * range.max(0.05)).sqrt();
        if closing && vx.abs() > stop_speed.max(0.35) {
            let excess = vx.abs() - stop_speed;
            return -vx.signum() * (excess + 0.15).min(1.0);
        }

        // Overdamped: modest position gain, heavy velocity damping, tight speed cap.
        let edge = (range / pad_half).clamp(0.0, 1.35);
        let pos_gain = 0.28 + edge * 0.22;
        let vel_gain = 0.95 + edge * 0.15;
        let cap = 0.7 + edge * 0.45;
        let mut target = dx * pos_gain;
        // Closing speed that can stop inside the remaining offset (~0.55 m/s²).
        let max_closing = stop_speed.min(cap);
        target = target.clamp(-max_closing, max_closing);
        // Bleed residual lateral speed without commanding a reverse hunt.
        if !closing {
            target -= vx * vel_gain * 0.35;
            target = target.clamp(-cap, cap);
        }
        return target;
    }

    if let Some(vx_target) = approach_funnel(dx, alt, vx, pad_half) {
        return vx_target;
    }

    if range < 1.0 && alt < 4.0 && vx.abs() < 0.3 {
        return 0.0;
    }

    if range < 2.5 {
        return (dx * 0.55 - vx * 0.7).clamp(-0.8, 0.8);
    }

    // Far-field: braking envelope sized for the ~36 m/s approach.
    let brake_accel = if range < 30.0 {
        2.2
    } else if range < 80.0 {
        1.6
    } else {
        1.35
    };
    let max_speed = if range < 20.0 {
        1.15
    } else if range < 50.0 {
        2.8
    } else if range < 120.0 {
        6.0
    } else if alt > 28.0 {
        18.0
    } else {
        7.0
    };

    let stopping_speed = (2.0 * brake_accel * range).sqrt();

    if alt > 28.0 && range > 90.0 {
        let descent_seconds = (alt / 1.5).clamp(6.0, 45.0);
        let glide_speed = (range / descent_seconds).min(18.0);
        return toward_pad * glide_speed.min(stopping_speed).min(max_speed);
    }

    toward_pad * stopping_speed.min(max_speed)
}

fn target_vertical_velocity(alt: f32, range: f32, vx: f32, pad_half: f32) -> f32 {
    let captured = ready_to_land(range, vx);

    // Emergency pull-up if terrain is close while still fast / off-center.
    if alt < 4.0 && (range > pad_half || vx.abs() > 2.0) {
        return -0.8;
    }
    if alt < 8.0 && range > 40.0 && vx.abs() > 8.0 {
        return -0.5;
    }

    // Keep descending while centering — faster as range shrinks, no long hover.
    if in_landing_corridor(alt, range, pad_half) && !captured {
        if alt < 2.5 {
            return -0.2;
        }
        let center = (1.0 - (range / (pad_half + 2.0)).clamp(0.0, 1.0)).powf(1.6);
        let slow = if alt < 8.0 { 0.18 } else { 0.35 };
        return slow + center * 0.7;
    }

    if range > 90.0 {
        if alt < 14.0 {
            -0.4
        } else if alt < 26.0 {
            0.05
        } else {
            (alt * 0.02).clamp(0.25, 0.9)
        }
    } else if range > 45.0 {
        if alt > 14.0 {
            0.45
        } else if alt < 6.0 {
            -0.15
        } else {
            0.15
        }
    } else if range > 18.0 {
        if alt > 10.0 {
            0.7
        } else if alt < 5.0 {
            0.05
        } else {
            0.2
        }
    } else if !captured {
        if alt < 3.0 {
            -0.2
        } else if alt < 9.0 {
            0.1
        } else if alt > 40.0 {
            1.0
        } else {
            (alt * 0.25).sqrt().min(1.8)
        }
    } else if alt > 40.0 {
        1.0
    } else if alt > 8.0 {
        (alt * 0.3).sqrt().min(2.1)
    } else if alt > 2.0 {
        (alt * 0.18).max(0.14)
    } else if alt > 0.8 {
        0.1
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
    let captured = ready_to_land(range, vel.x);

    let target_vx = target_horizontal_velocity(dx, alt, vel.x, pad_half);
    let target_vy = target_vertical_velocity(alt, range, vel.x, pad_half);
    let vx_err = target_vx - vel.x;
    let vy_err = target_vy - vel.y;

    let mut max_pitch = if alt > 25.0 {
        HIGH_ALT_MAX_PITCH
    } else {
        MAX_GUIDANCE_PITCH
    };
    if in_landing_corridor(alt, range, pad_half) && !captured {
        // Enough tilt to slide in, but taper near center to kill pitch oscillation.
        let edge = (range / pad_half).clamp(0.0, 1.0);
        max_pitch = (0.28 + edge * 0.35).min(max_pitch);
    }
    // High-speed approach needs aggressive pitch authority to brake in time.
    if vel.x.abs() > 12.0 && alt > 18.0 {
        max_pitch = max_pitch.max(HIGH_ALT_MAX_PITCH);
    }
    // Never pitch so far that we cannot arrest the current descent before impact.
    // max upward accel at pitch θ is (T/m)*cos(θ); require margin over gravity + braking.
    let thrust_accel = MAIN_THRUST_FORCE / LANDER_MASS;
    if vel.y > 1.0 && alt < 40.0 {
        let stop_accel = (vel.y * vel.y) / (2.0 * alt.max(0.5));
        let need_up = (MOON_GRAVITY + stop_accel * 1.15).min(thrust_accel * 0.98);
        let min_cos = (need_up / thrust_accel).clamp(0.35, 1.0);
        let pitch_cap = min_cos.acos();
        max_pitch = max_pitch.min(pitch_cap);
    }

    let mut pitch_gain = KP_PITCH;
    if vel.x.abs() > 10.0 {
        pitch_gain = pitch_gain.max(0.1);
    }
    if range < 250.0 && vel.x.abs() > 2.0 {
        pitch_gain = 0.075 + (250.0 - range.min(250.0)) / 250.0 * 0.055;
    }
    if alt < 35.0 && vel.x.abs() > 3.0 {
        pitch_gain = pitch_gain.max(0.09);
    }
    if alt < 12.0 && vel.x.abs() > 1.0 && range > 4.0 {
        pitch_gain = pitch_gain.max(0.1);
    }
    if in_landing_corridor(alt, range, pad_half) {
        let edge = (range / pad_half).clamp(0.0, 1.0);
        // Lower gain near center — high gain was the hover wiggle.
        pitch_gain = 0.06 + edge * 0.06;
    }
    // Moving away from pad center — brake back toward the middle.
    if range > 1.0 && dx.signum() != vel.x.signum() && vel.x.abs() > 0.35 {
        pitch_gain = pitch_gain.max(0.12);
    }

    let raw_target = (vx_err * pitch_gain).clamp(-max_pitch, max_pitch);
    let pitch_smooth = if range < 140.0
        || vx_err.abs() > 2.0
        || vel.x.abs() > 4.0
        || (range > 80.0 && alt > 30.0)
        || in_landing_corridor(alt, range, pad_half)
    {
        1.0
    } else {
        PITCH_SMOOTH
    };
    *smoothed_pitch += (raw_target - *smoothed_pitch) * pitch_smooth;
    let mut target_angle = *smoothed_pitch;

    // Once positionally captured, kill pitch so touchdown stays upright.
    if captured {
        let blend = if alt < 10.0 {
            (1.0 - alt / 10.0).clamp(0.35, 1.0)
        } else {
            0.35
        };
        target_angle *= 1.0 - blend;
        *smoothed_pitch = target_angle;
    } else if alt < 3.5 {
        let level = (1.0 - alt / 3.5).clamp(0.0, 1.0);
        target_angle *= 1.0 - level * 0.85;
    }
    if alt < 2.2 {
        target_angle = target_angle.clamp(-0.1, 0.1);
    }

    let (tilt_left, tilt_right) = attitude_thrust(lander, target_angle, KP_ANGLE, KD_ANGLE);

    let desired_vertical_accel = vy_err * KP_VY;
    let main_dir = lander.world_thrust_direction(&lander.main_thruster);
    let upward = upward_thrust_fraction(main_dir);
    let mut main = main_throttle_for_vertical(lander, desired_vertical_accel, 1.0);

    // Pitched braking steals vertical thrust — compensate when descending too fast.
    if upward < 0.9 && vel.y > target_vy + 0.8 {
        let hover = (HOVER_THROTTLE / upward.max(0.3)).min(1.0);
        main = main.max(hover);
    }

    if alt < 22.0 && vel.y > target_vy + 0.4 {
        main = main.max((HOVER_THROTTLE / upward.max(0.3)).min(1.0));
    }

    // Hard pull-up: commit full throttle when impact is imminent.
    if alt < 16.0 && vel.y > 3.0 {
        main = 1.0;
    }

    if in_landing_corridor(alt, range, pad_half) && !captured && alt < 5.0 && vel.y > 1.0 {
        // Arrest a dive near the surface while still translating; don't force a hover.
        let hover = (HOVER_THROTTLE / upward.max(0.45)).min(0.92);
        main = main.max(hover);
    }

    (main, tilt_left, tilt_right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameState, GameStatus, INITIAL_DESCENT_VY, INITIAL_ORBITAL_SPEED};
    use crate::physics::{sum_thrusters, PHYSICS_DT};
    use crate::world::{World, WORLD_MIN_X, WORLD_WIDTH};

    const SOFT_LANDING_VY: f32 = 3.0;
    const SOFT_LANDING_VX: f32 = 2.0;
    const SOFT_LANDING_ANGLE: f32 = 15.0_f32.to_radians();
    /// Soft landings should finish well inside the pad, not on the lip.
    const CENTER_LANDING_OFFSET: f32 = 2.0;

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

    fn is_soft_pad_landing(world: &World, lander: &Lander) -> bool {
        world.is_on_pad(lander.body.pos.x)
            && lander.body.vel.y.abs() <= SOFT_LANDING_VY
            && lander.body.vel.x.abs() <= SOFT_LANDING_VX
            && lander.body.angle.abs() <= SOFT_LANDING_ANGLE
    }

    fn simulate_autopilot(world: &World, lander: &mut Lander, max_steps: usize) -> SimState {
        let mut smoothed_pitch = lander.body.angle;

        for step in 0..max_steps {
            autopilot_physics_step(lander, world, &mut smoothed_pitch);

            if world.check_collision(&lander.hull_world) {
                let outcome = if is_soft_pad_landing(world, lander) {
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
        let initial_vel = Vec2::new(-INITIAL_ORBITAL_SPEED, INITIAL_DESCENT_VY);

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
        assert!(
            target_vx < -10.0,
            "expected strong leftward closure, got {target_vx}"
        );
    }

    #[test]
    fn descends_faster_when_more_centered() {
        let pad_half = 8.0;
        let vy_edge = target_vertical_velocity(6.0, 7.5, 1.5, pad_half);
        let vy_mid = target_vertical_velocity(6.0, 3.0, 0.4, pad_half);
        let vy_center = target_vertical_velocity(6.0, 0.5, 0.1, pad_half);
        assert!(
            vy_edge < vy_mid && vy_mid <= vy_center,
            "expected progressive descent edge={vy_edge} mid={vy_mid} center={vy_center}"
        );
        assert!(vy_edge > 0.1, "should keep sinking at the lip, got {vy_edge}");
        assert!(vy_center > 0.4, "expected committed descent once centered, got {vy_center}");
    }

    #[test]
    fn steers_toward_pad_center_on_final_approach() {
        let vx = target_horizontal_velocity(2.0, 3.0, 0.4, 8.0);
        assert!(
            vx > 0.5,
            "expected rightward closure toward center, got {vx}"
        );
    }

    #[test]
    fn steers_from_pad_edge_toward_center() {
        let vx = target_horizontal_velocity(-7.5, 8.0, 1.5, 8.0);
        assert!(
            vx < -0.6,
            "expected leftward closure from pad edge, got {vx}"
        );
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
    fn levels_out_before_touchdown() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);
        let mut smoothed_pitch = lander.body.angle;

        let mut max_low_alt_angle = 0.0f32;
        for _ in 0..20_000 {
            let alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
            if alt < 3.0 {
                max_low_alt_angle = max_low_alt_angle.max(lander.body.angle.abs());
            }
            autopilot_physics_step(&mut lander, &world, &mut smoothed_pitch);
            if world.check_collision(&lander.hull_world) {
                break;
            }
        }

        assert!(
            is_soft_pad_landing(&world, &lander),
            "expected soft pad landing, vel {:?} angle {}",
            lander.body.vel,
            lander.body.angle
        );
        assert!(
            lander.body.angle.abs() < SOFT_LANDING_ANGLE,
            "touchdown attitude too tipped: {} rad",
            lander.body.angle
        );
        assert!(
            max_low_alt_angle < 0.35,
            "expected near-upright below 3 m, got {max_low_alt_angle} rad"
        );
    }

    #[test]
    fn approach_trajectory_stays_in_bounds() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);
        let mut smoothed_pitch = lander.body.angle;

        let mut max_alt = 0.0f32;
        let mut min_x = f32::INFINITY;

        for _ in 0..20_000 {
            let alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
            max_alt = max_alt.max(alt);
            min_x = min_x.min(lander.body.pos.x);

            autopilot_physics_step(&mut lander, &world, &mut smoothed_pitch);
            if world.check_collision(&lander.hull_world) {
                break;
            }
        }

        assert!(max_alt < 100.0, "climbed too high: {max_alt} m");
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
        // Keep the delay inside the lander's recoverable envelope: at 36 m/s
        // approach with lunar T/W, ~3 s of free-fall already needs more altitude
        // than remains to null the descent.
        for _ in 0..90 {
            lander.body.apply_gravity(PHYSICS_DT);
            lander.body.integrate(PHYSICS_DT);
            lander.update_hull_world();
        }

        let alt_at_engage = world.altitude(lander.body.pos.x, lander.body.pos.y);
        assert!(
            alt_at_engage > 20.0,
            "test setup should still be recoverable, alt={alt_at_engage}"
        );

        let result = simulate_autopilot(&world, &mut lander, 20_000);
        assert_eq!(
            result.outcome,
            SimOutcome::Landed,
            "delayed AP failed: {:?} pos {:?} vel {:?} angle {}",
            result.outcome,
            result.pos,
            result.vel,
            result.angle
        );
        let dx = (result.pos.x - world.pad_center_x).abs();
        assert!(
            dx < CENTER_LANDING_OFFSET,
            "delayed AP landed off-center: {dx:.2} m"
        );
        assert!(
            result.pos.x >= WORLD_MIN_X,
            "delayed AP overshot left edge"
        );
    }

    #[test]
    fn soft_lands_near_pad_center_across_seeds() {
        let mut worst = 0.0f32;
        let mut worst_seed = 0u64;
        let mut crashes = 0usize;

        for seed in 0..200u64 {
            let mut game = GameState::new(seed);
            game.simulate_autopilot_until(|_| false);

            if game.status != GameStatus::Landed {
                crashes += 1;
                continue;
            }

            let dx = (game.lander.body.pos.x - game.world.pad_center_x).abs();
            if dx > worst {
                worst = dx;
                worst_seed = seed;
            }
            assert!(
                dx < CENTER_LANDING_OFFSET,
                "seed {seed} landed {dx:.2} m from pad center"
            );
            assert!(
                game.fuel > 200.0,
                "seed {seed} nearly dry on touchdown ({:.0})",
                game.fuel
            );
        }

        eprintln!("worst landing offset: seed {worst_seed} at {worst:.2}m ({crashes} crashes)");
        assert_eq!(crashes, 0, "{crashes} seeds crashed");
        assert!(
            worst < CENTER_LANDING_OFFSET,
            "worst offset {worst:.2}m on seed {worst_seed}"
        );
    }

    #[test]
    fn lands_near_pad_center_from_default_spawn() {
        for seed in [42u64, 1, 7, 99, 12345, 555] {
            let mut game = GameState::new(seed);
            game.simulate_autopilot_until(|_| false);
            assert_eq!(game.status, GameStatus::Landed, "seed {seed}");
            let dx = (game.lander.body.pos.x - game.world.pad_center_x).abs();
            assert!(
                dx < CENTER_LANDING_OFFSET,
                "seed {seed} landed {dx:.2} m from pad center"
            );
        }
    }

    #[test]
    fn soft_lands_on_pad_center_from_approach() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);

        let result = simulate_autopilot(&world, &mut lander, 20_000);

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
            dx < CENTER_LANDING_OFFSET,
            "expected landing near pad center, offset {dx:.2} m"
        );
        assert!(
            result.vel.y.abs() <= SOFT_LANDING_VY,
            "touchdown too hard vertically: {}",
            result.vel.y
        );
        assert!(
            result.vel.x.abs() <= SOFT_LANDING_VX,
            "touchdown too fast laterally: {}",
            result.vel.x
        );
        assert!(
            result.angle.abs() <= SOFT_LANDING_ANGLE,
            "touchdown attitude too tipped: {}",
            result.angle
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
        assert!(
            alt > 5.0 && alt < 16.0,
            "expected final approach altitude, got {alt}"
        );
        assert!(
            game.lander.throttle_main > 0.1,
            "expected main engine firing on approach"
        );
    }

    #[test]
    fn soft_lands_when_starting_above_pad() {
        let world = World::generate(42);
        let mut lander = Lander::new(Vec2::new(world.pad_center_x, world.pad_y - 35.0));
        lander.body.vel = Vec2::new(0.0, 0.4);
        lander.body.angle = 0.0;
        lander.update_hull_world();

        let result = simulate_autopilot(&world, &mut lander, 8_000);

        assert_eq!(
            result.outcome,
            SimOutcome::Landed,
            "vertical descent failed: {:?} pos {:?} vel {:?}",
            result.outcome,
            result.pos,
            result.vel
        );
        let dx = (result.pos.x - world.pad_center_x).abs();
        assert!(
            dx < CENTER_LANDING_OFFSET,
            "vertical start drifted off center: {dx:.2} m"
        );
    }

    #[test]
    fn final_approach_does_not_hover_hunt() {
        let world = World::generate(42);
        let mut lander = spawn_approach_lander(&world);
        let mut smoothed = lander.body.angle;
        let mut last_sign = 0.0f32;
        let mut crossings = 0usize;
        let mut corridor_step = None;
        let mut low_near_center_steps = 0usize;
        let mut land_step = None;

        for step in 0..20_000 {
            let dx = world.pad_center_x - lander.body.pos.x;
            let alt = world.altitude(lander.body.pos.x, lander.body.pos.y);
            let range = dx.abs();

            if range < 18.0 && alt < 26.0 && corridor_step.is_none() {
                corridor_step = Some(step);
            }
            // Count time spent low over the pad before touchdown — long dwell is the hover wiggle.
            if alt < 8.0 && range < 8.0 && land_step.is_none() {
                low_near_center_steps += 1;
                let s = dx.signum();
                if last_sign != 0.0 && s != last_sign && range > 0.25 {
                    crossings += 1;
                }
                if s != 0.0 {
                    last_sign = s;
                }
            }

            autopilot_physics_step(&mut lander, &world, &mut smoothed);
            if world.check_collision(&lander.hull_world) {
                land_step = Some(step);
                break;
            }
        }

        let land_step = land_step.expect("should touch down");
        assert!(
            is_soft_pad_landing(&world, &lander),
            "expected soft pad landing"
        );
        let dx = (lander.body.pos.x - world.pad_center_x).abs();
        assert!(dx < CENTER_LANDING_OFFSET, "off-center landing {dx:.2}m");

        assert!(
            crossings <= 1,
            "lateral hunt crossed pad center {crossings} times"
        );

        let corridor_s =
            (land_step - corridor_step.expect("should enter corridor")) as f32 / 60.0;
        let low_s = low_near_center_steps as f32 / 60.0;
        assert!(
            corridor_s < 30.0,
            "spent {corridor_s:.1}s in landing corridor before touchdown"
        );
        assert!(
            low_s < 18.0,
            "hovered/dwelled low for {low_s:.1}s before touchdown"
        );
    }
}
