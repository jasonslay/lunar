use bevy::prelude::*;
use bevy::render::camera::{CameraProjection, Projection, ScalingMode};
use bevy::render::view::RenderLayers;
use bevy::sprite::{Anchor, ColorMaterial, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use glam::Vec2;
use rand::Rng;

use crate::game::{GameState, GameStatus};
use crate::lander::{Lander, LANDER_BODY_FILL};
use crate::physics::{Thruster, PIXELS_PER_METER};
use crate::world::{World, SCREEN_HEIGHT, SCREEN_WIDTH, TERRAIN_MARKER_SPACING, TERRAIN_MAX_X, TERRAIN_MIN_X};

const GREEN: Color = Color::srgb(0.2, 1.0, 0.4);
const DIM_GREEN: Color = Color::srgb(0.1, 0.5, 0.2);
const BRIGHT_GREEN: Color = Color::srgb(0.4, 1.0, 0.6);
const LANDER_FILL: Color = Color::srgb(0.1, 0.48, 0.2);

const PLUME_CORE: Color = Color::srgb(1.0, 0.75, 0.2);
const PLUME_OUTER: Color = Color::srgb(1.0, 0.35, 0.1);
const DUST_BRIGHT: Color = Color::srgb(0.55, 1.0, 0.65);
const DUST_DIM: Color = Color::srgb(0.12, 0.42, 0.22);
/// Ray length for finding where exhaust meets terrain (much longer than the visible plume).
const MAX_DUST_RAY_M: f32 = 28.0;
/// Dust fades in as the nozzle gets within this distance of the impact point.
const DUST_FALLOFF_M: f32 = 20.0;
const STAR_DIM: Color = Color::srgb(0.7, 0.85, 1.0);
const STAR_BRIGHT: Color = Color::srgb(1.0, 1.0, 1.0);
const SKY_TOP: f32 = 4.0;
const SKY_HEIGHT: f32 = SCREEN_HEIGHT - SKY_TOP;
const HUD_MARGIN: f32 = 14.0;
const HUD_BG_WIDTH: f32 = 210.0;
const HUD_BG_HEIGHT: f32 = 172.0;
const HUD_LINE_COUNT: usize = 7;
const HUD_BG_PAD: f32 = 10.0;
const HUD_FONT_SIZE: f32 = 18.0;
const HUD_LINE_STEP: f32 = 22.0;
const STATUS_FONT_SIZE: f32 = 20.0;
const STATUS_BG_WIDTH: f32 = 760.0;
const STATUS_BG_HEIGHT: f32 = 52.0;
const STATUS_BELOW_HORIZON: f32 = 40.0;
const PANEL_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.94);
const UI_LAYERS: RenderLayers = RenderLayers::layer(1);
const CRATER_HASH_SCALE: f32 = 43_758.547;

#[derive(Component)]
pub struct HudPanel;

#[derive(Component)]
pub struct HudLine(pub usize);

#[derive(Component)]
pub struct StatusPanel;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct LanderBody;

#[derive(Component)]
struct UiCamera;

pub fn setup_camera(mut commands: Commands) {
    let mut world_projection = OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        },
        ..OrthographicProjection::default_2d()
    };
    world_projection.update(SCREEN_WIDTH, SCREEN_HEIGHT);

    commands.spawn((Camera2d, Projection::Orthographic(world_projection)));

    // UI renders at window pixel resolution so text stays crisp when resized.
    let mut ui_projection = OrthographicProjection {
        scaling_mode: ScalingMode::WindowSize,
        ..OrthographicProjection::default_2d()
    };
    ui_projection.update(SCREEN_WIDTH, SCREEN_HEIGHT);

    commands.spawn((
        UiCamera,
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Orthographic(ui_projection),
        UI_LAYERS,
    ));
}

pub fn setup_hud(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let hud_left = -SCREEN_WIDTH * 0.5 + HUD_MARGIN;
    let hud_top = SCREEN_HEIGHT * 0.5 - HUD_MARGIN;
    let hud_bg_center = Vec3::new(
        hud_left - HUD_BG_PAD + HUD_BG_WIDTH * 0.5,
        hud_top + HUD_BG_PAD - HUD_BG_HEIGHT * 0.5,
        0.0,
    );

    commands.spawn((
        HudPanel,
        Mesh2d(meshes.add(Rectangle::new(HUD_BG_WIDTH, HUD_BG_HEIGHT))),
        MeshMaterial2d(materials.add(PANEL_COLOR)),
        Transform::from_translation(hud_bg_center),
        UI_LAYERS,
    ));

    for i in 0..HUD_LINE_COUNT {
        commands.spawn((
            HudLine(i),
            Text2d::new(""),
            TextFont {
                font_size: HUD_FONT_SIZE,
                ..default()
            },
            TextColor(GREEN),
            Anchor::TopLeft,
            Transform::from_xyz(hud_left, hud_top - i as f32 * HUD_LINE_STEP, 1.0),
            UI_LAYERS,
        ));
    }

    commands.spawn((
        StatusPanel,
        Mesh2d(meshes.add(Rectangle::new(STATUS_BG_WIDTH, STATUS_BG_HEIGHT))),
        MeshMaterial2d(materials.add(PANEL_COLOR)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
        UI_LAYERS,
    ));

    commands.spawn((
        StatusText,
        Text2d::new(""),
        TextFont {
            font_size: STATUS_FONT_SIZE,
            ..default()
        },
        TextColor(BRIGHT_GREEN),
        Anchor::Center,
        Transform::from_xyz(0.0, 0.0, 1.0),
        Visibility::Hidden,
        UI_LAYERS,
    ));
}

fn lander_fill_mesh(vertices: &[Vec2]) -> Mesh {
    let positions: Vec<[f32; 3]> = vertices
        .iter()
        .map(|v| [v.x, -v.y, 0.0])
        .collect();
    let mut indices = Vec::with_capacity((vertices.len() - 2) * 3);
    for i in 1..vertices.len() - 1 {
        indices.extend_from_slice(&[0, i as u32, (i + 1) as u32]);
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

pub fn setup_lander(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        LanderBody,
        Mesh2d(meshes.add(lander_fill_mesh(&LANDER_BODY_FILL))),
        MeshMaterial2d(materials.add(LANDER_FILL)),
        Transform::default(),
    ));
}

pub fn update_lander_body(mut lander: Query<&mut Transform, With<LanderBody>>, game: Res<GameState>) {
    let cam = game.camera_offset();
    let pos = game.lander.body.pos;
    let screen = world_to_screen(pos, cam);
    let bevy_pos = to_bevy(screen);

    let Ok(mut transform) = lander.get_single_mut() else {
        return;
    };

    transform.translation = Vec3::new(bevy_pos.x, bevy_pos.y, 0.5);
    transform.rotation = Quat::from_rotation_z(-game.lander.body.angle);
    transform.scale = Vec3::new(PIXELS_PER_METER, PIXELS_PER_METER, 1.0);
}

fn status_y_below_horizon(game: &GameState) -> f32 {
    let cam = game.camera_offset();
    let world_x = game.lander.body.pos.x;
    let horizon_screen_y =
        (game.world.height_at(world_x) - cam.y) * PIXELS_PER_METER;
    let status_screen_y =
        (horizon_screen_y + STATUS_BELOW_HORIZON).min(SCREEN_HEIGHT - 28.0);
    to_bevy(Vec2::new(SCREEN_WIDTH * 0.5, status_screen_y)).y
}

fn world_to_screen(world_m: Vec2, cam_m: Vec2) -> Vec2 {
    Vec2::new(
        (world_m.x - cam_m.x) * PIXELS_PER_METER,
        (world_m.y - cam_m.y) * PIXELS_PER_METER,
    )
}

fn to_bevy(screen: Vec2) -> Vec2 {
    Vec2::new(
        screen.x - SCREEN_WIDTH * 0.5,
        SCREEN_HEIGHT * 0.5 - screen.y,
    )
}

/// Scale UI to match the letterboxed game viewport (1.0 at the design 800×600 size).
fn ui_viewport_scale(window_w: f32, window_h: f32) -> f32 {
    (window_w / SCREEN_WIDTH).min(window_h / SCREEN_HEIGHT)
}

/// Map a world-camera bevy coordinate into UI-camera space (letterboxed 800×600 → window).
fn map_world_bevy_to_ui(world_bevy: Vec2, window_w: f32, window_h: f32) -> Vec2 {
    let scale = ui_viewport_scale(window_w, window_h);
    let viewport_w = SCREEN_WIDTH * scale;
    let viewport_h = SCREEN_HEIGHT * scale;
    let offset_x = (window_w - viewport_w) * 0.5;
    let offset_y = (window_h - viewport_h) * 0.5;

    let screen_x = world_bevy.x + SCREEN_WIDTH * 0.5;
    let screen_y = SCREEN_HEIGHT * 0.5 - world_bevy.y;

    let px = offset_x + screen_x * scale;
    let py = offset_y + screen_y * scale;

    Vec2::new(px - window_w * 0.5, window_h * 0.5 - py)
}

fn layout_hud_lines(
    hud_left: f32,
    hud_top: f32,
    line_step: f32,
    font_size: f32,
    hud: &mut Query<
        (&HudLine, &mut Transform, &mut Text2d, &mut TextFont),
        (
            With<HudLine>,
            Without<HudPanel>,
            Without<StatusText>,
            Without<StatusPanel>,
        ),
    >,
) {
    for (HudLine(i), mut transform, _, mut font) in hud {
        transform.translation = Vec3::new(hud_left, hud_top - *i as f32 * line_step, 1.0);
        font.font_size = font_size;
    }
}

fn line_2d(gizmos: &mut Gizmos, a: Vec2, b: Vec2, color: Color) {
    gizmos.line_2d(to_bevy(a), to_bevy(b), color);
}

pub fn draw_world(mut gizmos: Gizmos, game: Res<GameState>, time: Res<Time>) {
    let cam = game.camera_offset();

    draw_stars(&mut gizmos, &game.world, cam);
    draw_terrain(&mut gizmos, &game.world, cam);
    draw_lander(
        &mut gizmos,
        &game.lander,
        cam,
        game.status == GameStatus::Flying,
        time.elapsed_secs(),
    );

    if game.status == GameStatus::Flying {
        draw_dust_kickup(
            &mut gizmos,
            &game.world,
            &game.lander,
            &game.lander.main_thruster,
            game.lander.throttle_main,
            cam,
            time.elapsed_secs(),
        );
    }
}

fn draw_stars(gizmos: &mut Gizmos, world: &World, cam: Vec2) {
    for star in &world.stars {
        let screen_x = star.nx * SCREEN_WIDTH;
        let screen_y = SKY_TOP + star.ny * SKY_HEIGHT;

        let world_x = cam.x + screen_x / PIXELS_PER_METER;
        let horizon_y = (world.height_at(world_x) - cam.y) * PIXELS_PER_METER;

        if horizon_y < SKY_TOP + 8.0 || screen_y >= horizon_y - 1.0 {
            continue;
        }

        draw_star_dot(gizmos, screen_x, screen_y, star.brightness);
    }
}

fn draw_star_dot(gizmos: &mut Gizmos, x: f32, y: f32, brightness: f32) {
    let color = if brightness > 0.72 {
        STAR_BRIGHT
    } else {
        STAR_DIM
    };
    let size = 1.0 + brightness * 1.0;
    let c = to_bevy(Vec2::new(x, y));
    line_2d(gizmos, Vec2::new(x - size, y), Vec2::new(x + size, y), color);
    line_2d(gizmos, Vec2::new(x, y - size), Vec2::new(x, y + size), color);
    if brightness > 0.85 {
        gizmos.circle_2d(c, 0.6, color);
    }
}

fn draw_terrain(gizmos: &mut Gizmos, world: &World, cam: Vec2) {
    let view_left = cam.x;
    let view_right = cam.x + SCREEN_WIDTH / PIXELS_PER_METER;
    let terrain_left = world.terrain[0].x;
    let terrain_right = world.terrain[world.terrain.len() - 1].x;

    for window in world.terrain.windows(2) {
        let a = world_to_screen(window[0], cam);
        let b = world_to_screen(window[1], cam);

        let seg_on_pad = window[0].x >= world.pad_start_x && window[1].x <= world.pad_end_x;
        let color = if seg_on_pad { BRIGHT_GREEN } else { GREEN };

        line_2d(gizmos, a, b, color);
    }

    if view_left < terrain_left {
        let y = world.height_at(view_left);
        line_2d(
            gizmos,
            world_to_screen(Vec2::new(view_left, y), cam),
            world_to_screen(Vec2::new(terrain_left, y), cam),
            GREEN,
        );
    }
    if view_right > terrain_right {
        let y = world.height_at(terrain_right);
        line_2d(
            gizmos,
            world_to_screen(Vec2::new(terrain_right, y), cam),
            world_to_screen(Vec2::new(view_right, y), cam),
            GREEN,
        );
    }

    draw_distance_craters(gizmos, world, cam);

    let pad_left = world_to_screen(Vec2::new(world.pad_start_x, world.pad_y), cam);
    let pad_right = world_to_screen(Vec2::new(world.pad_end_x, world.pad_y), cam);
    let marker_h = 1.0 * PIXELS_PER_METER;
    line_2d(
        gizmos,
        Vec2::new(pad_left.x, pad_left.y - marker_h),
        Vec2::new(pad_left.x, pad_left.y + marker_h),
        BRIGHT_GREEN,
    );
    line_2d(
        gizmos,
        Vec2::new(pad_right.x, pad_right.y - marker_h),
        Vec2::new(pad_right.x, pad_right.y + marker_h),
        BRIGHT_GREEN,
    );
}

fn is_spacing_multiple(x: f32, spacing: f32) -> bool {
    if spacing <= 0.0 {
        return false;
    }
    let n = (x / spacing).round();
    (n * spacing - x).abs() < spacing * 0.05
}

fn crater_radius_px(x: f32, every_100: bool, every_50: bool) -> f32 {
    let hash = ((x * 12.9898).sin() * CRATER_HASH_SCALE).fract();
    let wobble = 0.85 + hash * 0.3;

    if every_100 {
        20.0 * wobble
    } else if every_50 {
        12.0 * wobble
    } else {
        5.5 * wobble
    }
}

fn terrain_frame_at(
    world: &World,
    x: f32,
    cam: Vec2,
    half_width_m: f32,
) -> (Vec2, Vec2, Vec2, Vec2) {
    let x0 = (x - half_width_m).max(TERRAIN_MIN_X);
    let x1 = (x + half_width_m).min(TERRAIN_MAX_X);
    let rim_left = world_to_screen(Vec2::new(x0, world.height_at(x0)), cam);
    let rim_right = world_to_screen(Vec2::new(x1, world.height_at(x1)), cam);
    let tangent = (rim_right - rim_left).normalize_or_zero();
    let mut inward = Vec2::new(-tangent.y, tangent.x);
    if inward.dot(Vec2::Y) < 0.0 {
        inward = -inward;
    }
    (rim_left, rim_right, tangent, inward)
}

#[allow(clippy::too_many_arguments)]
fn draw_aligned_crater(
    gizmos: &mut Gizmos,
    rim_left: Vec2,
    rim_right: Vec2,
    inward: Vec2,
    radius_px: f32,
    embed_frac: f32,
    depth_frac: f32,
    color: Color,
    segments: usize,
) {
    if radius_px < 2.0 {
        return;
    }

    let base_embed = radius_px * embed_frac;
    let max_depth = radius_px * depth_frac;
    let mut prev = None;

    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let along_rim = rim_left.lerp(rim_right, t);
        let bowl = (std::f32::consts::PI * t).sin();
        let p = along_rim + inward * (base_embed + max_depth * bowl);

        if let Some(prev) = prev {
            line_2d(gizmos, prev, p, color);
        }
        prev = Some(p);
    }
}

fn draw_distance_craters(gizmos: &mut Gizmos, world: &World, cam: Vec2) {
    let view_left = cam.x;
    let view_right = cam.x + SCREEN_WIDTH / PIXELS_PER_METER;
    let mut x = (view_left / TERRAIN_MARKER_SPACING).floor() * TERRAIN_MARKER_SPACING;

    while x <= view_right {
        if (TERRAIN_MIN_X..=TERRAIN_MAX_X).contains(&x) && !world.is_on_pad(x) {
            let every_100 = is_spacing_multiple(x, 100.0);
            let every_50 = is_spacing_multiple(x, 50.0);
            let radius = crater_radius_px(x, every_100, every_50);
            let half_width_m = (radius / PIXELS_PER_METER) * 0.92;

            let (rim_left, rim_right, _tangent, inward) =
                terrain_frame_at(world, x, cam, half_width_m);

            if rim_left.x < -40.0 && rim_right.x < -40.0
                || rim_left.x > SCREEN_WIDTH + 40.0 && rim_right.x > SCREEN_WIDTH + 40.0
            {
                x += TERRAIN_MARKER_SPACING;
                continue;
            }

            let color = if every_100 {
                BRIGHT_GREEN
            } else if every_50 {
                GREEN
            } else {
                DIM_GREEN
            };

            let hash = ((x * 12.9898).sin() * CRATER_HASH_SCALE).fract();
            let embed = 0.22 + hash * 0.12;
            let depth = 0.48 + hash * 0.1;

            draw_aligned_crater(
                gizmos,
                rim_left,
                rim_right,
                inward,
                radius,
                embed,
                depth,
                color,
                14,
            );

            if every_50 {
                let center = (rim_left + rim_right) * 0.5;
                draw_aligned_crater(
                    gizmos,
                    rim_left.lerp(center, 0.45),
                    rim_right.lerp(center, 0.45),
                    inward,
                    radius * 0.55,
                    embed * 1.1,
                    depth * 0.85,
                    color,
                    10,
                );
            }

            if every_100 {
                let center = (rim_left + rim_right) * 0.5;
                draw_aligned_crater(
                    gizmos,
                    rim_left.lerp(center, 0.62),
                    rim_right.lerp(center, 0.62),
                    inward,
                    radius * 0.3,
                    embed * 1.2,
                    depth * 0.7,
                    color,
                    8,
                );
                line_2d(gizmos, rim_left, rim_right, color);
            }
        }
        x += TERRAIN_MARKER_SPACING;
    }
}

fn draw_lander(
    gizmos: &mut Gizmos,
    lander: &Lander,
    cam: Vec2,
    show_plumes: bool,
    elapsed: f32,
) {
    let pos = lander.body.pos;
    let rot = |local: Vec2| pos + lander.body.rotate_local_to_world(local);

    if show_plumes {
        draw_thruster_plume(
            gizmos,
            lander,
            &lander.main_thruster,
            lander.throttle_main,
            2.8,
            5,
            cam,
            elapsed,
            0.0,
        );
        draw_thruster_plume(
            gizmos,
            lander,
            &lander.rotate_cw,
            lander.throttle_tilt_right,
            0.9,
            3,
            cam,
            elapsed,
            1.7,
        );
        draw_thruster_plume(
            gizmos,
            lander,
            &lander.rotate_ccw,
            lander.throttle_tilt_left,
            0.9,
            3,
            cam,
            elapsed,
            3.1,
        );
    }

    for (a, b) in &lander.frame_local {
        let a = world_to_screen(rot(*a), cam);
        let b = world_to_screen(rot(*b), cam);
        line_2d(gizmos, a, b, GREEN);
    }

    let engine = world_to_screen(lander.thruster_world_positions()[0], cam);
    let nozzle_half = 0.22 * PIXELS_PER_METER;
    line_2d(
        gizmos,
        Vec2::new(engine.x - nozzle_half, engine.y),
        Vec2::new(engine.x + nozzle_half, engine.y),
        BRIGHT_GREEN,
    );

    let com = world_to_screen(lander.body.pos, cam);
    gizmos.circle_2d(to_bevy(com), 2.0, DIM_GREEN);
}

#[allow(clippy::too_many_arguments)]
fn draw_thruster_plume(
    gizmos: &mut Gizmos,
    lander: &Lander,
    thruster: &Thruster,
    throttle: f32,
    base_length_m: f32,
    streaks: usize,
    cam: Vec2,
    elapsed: f32,
    phase: f32,
) {
    if throttle <= 0.0 {
        return;
    }

    let mut rng = rand::thread_rng();
    let origin = world_to_screen(lander.thruster_world_position(thruster), cam);
    let thrust = lander.world_thrust_direction(thruster);
    let exhaust = Vec2::new(-thrust.x, -thrust.y);
    let perp = Vec2::new(-exhaust.y, exhaust.x);
    let flicker = (elapsed * 24.0 + phase).sin() * 0.12 + 1.0;
    let length_px = base_length_m * throttle * flicker * PIXELS_PER_METER;

    for i in 0..streaks {
        let t = if streaks == 1 {
            0.0
        } else {
            (i as f32 / (streaks - 1) as f32) - 0.5
        };
        let spread = perp * t * rng.gen_range(0.15..0.35) * PIXELS_PER_METER;
        let streak_len = length_px * rng.gen_range(0.75..1.0);
        let jitter =
            Vec2::new(rng.gen_range(-0.08..0.08), rng.gen_range(-0.08..0.08)) * PIXELS_PER_METER;
        let end = origin + exhaust * streak_len + spread + jitter;
        let color = if base_length_m > 2.0 && rng.gen_range(0.0..1.0) > 0.35 {
            PLUME_CORE
        } else {
            PLUME_OUTER
        };
        line_2d(gizmos, origin, end, color);
    }
}

/// March along the exhaust ray until it intersects the terrain polyline.
fn exhaust_ground_hit(
    world: &World,
    origin: Vec2,
    exhaust: Vec2,
    max_length_m: f32,
) -> Option<Vec2> {
    let dir = exhaust.normalize_or_zero();
    if dir.length_squared() < 1e-8 || max_length_m <= 0.0 {
        return None;
    }

    const STEP_M: f32 = 0.04;
    let steps = ((max_length_m / STEP_M).ceil() as usize).max(1);

    for i in 1..=steps {
        let t = (i as f32 * STEP_M).min(max_length_m);
        let p = origin + dir * t;
        let ground = world.height_at(p.x);
        if p.y >= ground {
            return Some(Vec2::new(p.x, ground));
        }
    }

    None
}

fn draw_dust_kickup(
    gizmos: &mut Gizmos,
    world: &World,
    lander: &Lander,
    thruster: &Thruster,
    throttle: f32,
    cam: Vec2,
    elapsed: f32,
) {
    if throttle <= 0.0 {
        return;
    }

    let origin = lander.thruster_world_position(thruster);
    let exhaust = -lander.world_thrust_direction(thruster);

    // Exhaust must point generally downward to reach the surface.
    if exhaust.y <= 0.15 {
        return;
    }

    let Some(hit) = exhaust_ground_hit(world, origin, exhaust, MAX_DUST_RAY_M) else {
        return;
    };

    let travel = (hit - origin).length();
    let proximity = (1.0 - travel / DUST_FALLOFF_M).clamp(0.0, 1.0);
    let pad_scale = if world.is_on_pad(hit.x) { 0.55 } else { 1.0 };
    let flicker = (elapsed * 18.0).sin() * 0.08 + 1.0;
    let intensity = throttle * proximity * pad_scale * flicker;
    if intensity < 0.08 {
        return;
    }

    let dx = 0.5;
    let x0 = (hit.x - dx).max(TERRAIN_MIN_X);
    let x1 = (hit.x + dx).min(TERRAIN_MAX_X);
    let rim_left = world_to_screen(Vec2::new(x0, world.height_at(x0)), cam);
    let rim_right = world_to_screen(Vec2::new(x1, world.height_at(x1)), cam);
    let tangent = (rim_right - rim_left).normalize_or_zero();
    let mut up = Vec2::new(-tangent.y, tangent.x);
    if up.y > 0.0 {
        up = -up;
    }

    let hit_screen = world_to_screen(hit, cam);
    let zone_half_px = (0.5 + intensity * 1.4) * PIXELS_PER_METER;
    let streak_count = (intensity * 28.0).ceil() as usize + 10;
    let mut rng = rand::thread_rng();

    for _ in 0..streak_count {
        let along = rng.gen_range(-zone_half_px..zone_half_px);
        let surface_lift = rng.gen_range(0.0..0.08) * PIXELS_PER_METER;
        let start = hit_screen + tangent * along + up * surface_lift;

        let kind: u8 = rng.gen_range(0..10);
        let (dir, len_m, color) = if kind < 4 {
            // Low surface spray along the ground, both directions.
            let sign = if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
            let dir = (tangent * sign + up * rng.gen_range(0.08..0.22)).normalize_or_zero();
            let len_m = rng.gen_range(0.6..2.2) * intensity;
            (dir, len_m, if rng.gen_bool(0.4) { GREEN } else { DIM_GREEN })
        } else if kind < 7 {
            // Lofted kick-up — steep rise with sideways drift.
            let dir = (up * rng.gen_range(0.55..1.0) + tangent * rng.gen_range(-0.65..0.65))
                .normalize_or_zero();
            let len_m = rng.gen_range(1.0..3.2) * intensity;
            (
                dir,
                len_m,
                if rng.gen_bool(0.35) {
                    DUST_BRIGHT
                } else {
                    BRIGHT_GREEN
                },
            )
        } else {
            // Billowing arc: up then shear with the surface tangent.
            let rise = (up * rng.gen_range(0.45..0.85) + tangent * rng.gen_range(-0.35..0.35))
                .normalize_or_zero();
            let len_m = rng.gen_range(0.8..2.4) * intensity;
            let mid_m = len_m * rng.gen_range(0.35..0.55);
            let end_m = len_m - mid_m;
            let mid = start + rise * mid_m * PIXELS_PER_METER;
            let shear = (tangent * rng.gen_range(-0.9..0.9) + up * rng.gen_range(0.1..0.35))
                .normalize_or_zero();
            let end = mid + shear * end_m * PIXELS_PER_METER;
            line_2d(gizmos, start, mid, GREEN);
            line_2d(gizmos, mid, end, DUST_DIM);
            continue;
        };

        if dir.length_squared() < 1e-6 {
            continue;
        }

        let end = start + dir * len_m * PIXELS_PER_METER;
        line_2d(gizmos, start, end, color);
    }
}

#[cfg(test)]
mod dust_tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn exhaust_ray_finds_ground_below_nozzle() {
        let world = World::generate(42);
        let x = 120.0;
        let ground_y = world.height_at(x);
        let origin = Vec2::new(x, ground_y - 4.0);
        let hit = exhaust_ground_hit(&world, origin, Vec2::new(0.0, 1.0), MAX_DUST_RAY_M);
        let hit = hit.expect("exhaust should hit terrain");
        assert!((hit.y - ground_y).abs() < 0.15);
    }

    #[test]
    fn short_plume_ray_misses_ground_that_long_ray_reaches() {
        let world = World::generate(42);
        let x = 120.0;
        let ground_y = world.height_at(x);
        let origin = Vec2::new(x, ground_y - 4.0);
        assert!(exhaust_ground_hit(&world, origin, Vec2::new(0.0, 1.0), 2.8).is_none());
        assert!(exhaust_ground_hit(&world, origin, Vec2::new(0.0, 1.0), MAX_DUST_RAY_M).is_some());
    }
}

#[allow(clippy::type_complexity)]
pub fn update_hud(
    time: Res<Time>,
    mut fps_smooth: Local<Option<f32>>,
    game: Res<GameState>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut hud_panel: Query<
        &mut Transform,
        (
            With<HudPanel>,
            Without<HudLine>,
            Without<StatusText>,
            Without<StatusPanel>,
        ),
    >,
    mut hud: Query<
        (&HudLine, &mut Transform, &mut Text2d, &mut TextFont),
        (
            With<HudLine>,
            Without<HudPanel>,
            Without<StatusText>,
            Without<StatusPanel>,
        ),
    >,
    mut status: Query<
        (&mut Text2d, &mut Transform, &mut Visibility, &mut TextFont),
        (
            With<StatusText>,
            Without<StatusPanel>,
            Without<HudLine>,
            Without<HudPanel>,
        ),
    >,
    mut status_panel: Query<
        (&mut Transform, &mut Visibility),
        (
            With<StatusPanel>,
            Without<StatusText>,
            Without<HudLine>,
            Without<HudPanel>,
        ),
    >,
) {
    let Ok(window) = window.get_single() else {
        return;
    };
    let window_w = window.width();
    let window_h = window.height();
    let scale = ui_viewport_scale(window_w, window_h);
    let margin = HUD_MARGIN * scale;
    let pad = HUD_BG_PAD * scale;
    let bg_w = HUD_BG_WIDTH * scale;
    let bg_h = HUD_BG_HEIGHT * scale;
    let line_step = HUD_LINE_STEP * scale;
    let hud_font = HUD_FONT_SIZE * scale;
    let status_font = STATUS_FONT_SIZE * scale;

    let hud_left = -window_w * 0.5 + margin;
    let hud_top = window_h * 0.5 - margin;

    if let Ok(mut panel) = hud_panel.get_single_mut() {
        panel.translation = Vec3::new(
            hud_left - pad + bg_w * 0.5,
            hud_top + pad - bg_h * 0.5,
            0.0,
        );
        panel.scale = Vec3::splat(scale);
    }

    layout_hud_lines(hud_left, hud_top, line_step, hud_font, &mut hud);

    let alt = game.world.clearance_above_terrain(&game.lander.hull_world);
    let vy = game.lander.body.vel.y;
    let vx = game.lander.body.vel.x;
    let angle_deg = game.lander.body.angle.to_degrees();
    let fuel = game.fuel;
    let ap = if game.autopilot { "ON " } else { "OFF" };

    let dt = time.delta_secs();
    if dt > 0.0 {
        let instant_fps = 1.0 / dt;
        *fps_smooth = Some(match *fps_smooth {
            Some(smooth) => smooth * 0.9 + instant_fps * 0.1,
            None => instant_fps,
        });
    }
    let fps = fps_smooth.unwrap_or(0.0);

    let lines = [
        format!("ALT  {:>6.0} m", alt),
        format!("VVEL {:>6.1} m/s", vy),
        format!("HVEL {:>6.1} m/s", vx),
        format!("ANG  {:>6.1} deg", angle_deg),
        format!("FUEL {:>6.0} kg", fuel),
        format!("AP   {:>6}", ap),
        format!("FPS  {:>6.0}", fps),
    ];

    for (HudLine(i), _, mut text, _) in &mut hud {
        if let Some(line) = lines.get(*i) {
            text.0 = line.clone();
        }
    }

    let status_world_y = status_y_below_horizon(&game);
    let status_ui = map_world_bevy_to_ui(Vec2::new(0.0, status_world_y), window_w, window_h);

    if let Ok((mut text, mut transform, mut vis, mut font)) = status.get_single_mut() {
        transform.translation.x = status_ui.x;
        transform.translation.y = status_ui.y;
        font.font_size = status_font;

        match game.status {
            GameStatus::Flying => {
                *vis = Visibility::Hidden;
            }
            GameStatus::Landed => {
                **text = "LANDED - Press R to retry, N for new terrain".into();
                *vis = Visibility::Visible;
            }
            GameStatus::Crashed => {
                **text = "CRASHED - Press R to retry, N for new terrain".into();
                *vis = Visibility::Visible;
            }
        }
    }

    if let Ok((mut transform, mut vis)) = status_panel.get_single_mut() {
        transform.translation.x = status_ui.x;
        transform.translation.y = status_ui.y;
        transform.scale = Vec3::splat(scale);
        *vis = if game.status == GameStatus::Flying {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}
