use bevy::prelude::*;
use bevy::render::camera::{CameraProjection, Projection, ScalingMode};
use bevy::render::view::RenderLayers;
use bevy::sprite::{Anchor, ColorMaterial, MeshMaterial2d};
use glam::Vec2;
use rand::Rng;

use crate::game::{GameState, GameStatus};
use crate::lander::Lander;
use crate::physics::{Thruster, PIXELS_PER_METER};
use crate::world::{World, SCREEN_HEIGHT, SCREEN_WIDTH, TERRAIN_MARKER_SPACING, WORLD_WIDTH};

const GREEN: Color = Color::srgb(0.2, 1.0, 0.4);
const DIM_GREEN: Color = Color::srgb(0.1, 0.5, 0.2);
const BRIGHT_GREEN: Color = Color::srgb(0.4, 1.0, 0.6);
const LANDER_FILL: Color = Color::srgb(0.1, 0.48, 0.2);
const PLUME_CORE: Color = Color::srgb(1.0, 0.75, 0.2);
const PLUME_OUTER: Color = Color::srgb(1.0, 0.35, 0.1);
const STAR_DIM: Color = Color::srgb(0.7, 0.85, 1.0);
const STAR_BRIGHT: Color = Color::srgb(1.0, 1.0, 1.0);
const SKY_TOP: f32 = 4.0;
const SKY_HEIGHT: f32 = SCREEN_HEIGHT - SKY_TOP;
const HUD_MARGIN: f32 = 14.0;
const HUD_BG_WIDTH: f32 = 210.0;
const HUD_BG_HEIGHT: f32 = 172.0;
const HUD_LINE_COUNT: usize = 7;
const HUD_BG_PAD: f32 = 10.0;
const STATUS_BG_WIDTH: f32 = 760.0;
const STATUS_BG_HEIGHT: f32 = 52.0;
const STATUS_BELOW_HORIZON: f32 = 40.0;
const PANEL_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.94);
const UI_LAYERS: RenderLayers = RenderLayers::layer(1);

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

/// Hull outline in local meters (clockwise).
const LANDER_FILL_LOCAL: [Vec2; 6] = [
    Vec2::new(0.0, -2.0),
    Vec2::new(1.0, -0.75),
    Vec2::new(2.25, 1.5),
    Vec2::new(0.0, 1.25),
    Vec2::new(-2.25, 1.5),
    Vec2::new(-1.0, -0.75),
];

pub fn setup_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        },
        ..OrthographicProjection::default_2d()
    };
    projection.update(SCREEN_WIDTH, SCREEN_HEIGHT);
    let projection = Projection::Orthographic(projection);

    commands.spawn((Camera2d, projection.clone()));

    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        projection,
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
                font_size: 18.0,
                ..default()
            },
            TextColor(GREEN),
            Anchor::TopLeft,
            Transform::from_xyz(hud_left, hud_top - i as f32 * 22.0, 1.0),
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
            font_size: 20.0,
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
        Mesh2d(meshes.add(lander_fill_mesh(&LANDER_FILL_LOCAL))),
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
    let size = 1.5 + brightness * 1.5;
    let c = to_bevy(Vec2::new(x, y));
    line_2d(gizmos, Vec2::new(x - size, y), Vec2::new(x + size, y), color);
    line_2d(gizmos, Vec2::new(x, y - size), Vec2::new(x, y + size), color);
    if brightness > 0.85 {
        gizmos.circle_2d(c, 1.0, color);
    }
}

fn draw_terrain(gizmos: &mut Gizmos, world: &World, cam: Vec2) {
    for window in world.terrain.windows(2) {
        let a = world_to_screen(window[0], cam);
        let b = world_to_screen(window[1], cam);

        let seg_on_pad = window[0].x >= world.pad_start_x && window[1].x <= world.pad_end_x;
        let color = if seg_on_pad { BRIGHT_GREEN } else { GREEN };

        line_2d(gizmos, a, b, color);
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
    let hash = ((x * 12.9898).sin() * 43758.5453).fract();
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
    let x0 = (x - half_width_m).max(0.0);
    let x1 = (x + half_width_m).min(WORLD_WIDTH);
    let rim_left = world_to_screen(Vec2::new(x0, world.height_at(x0)), cam);
    let rim_right = world_to_screen(Vec2::new(x1, world.height_at(x1)), cam);
    let tangent = (rim_right - rim_left).normalize_or_zero();
    let mut inward = Vec2::new(-tangent.y, tangent.x);
    if inward.dot(Vec2::Y) < 0.0 {
        inward = -inward;
    }
    (rim_left, rim_right, tangent, inward)
}

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
        if x >= 0.0 && x <= WORLD_WIDTH && !world.is_on_pad(x) {
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

            let hash = ((x * 12.9898).sin() * 43758.5453).fract();
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
    let nozzle_half = 0.5 * PIXELS_PER_METER;
    line_2d(
        gizmos,
        Vec2::new(engine.x - nozzle_half, engine.y),
        Vec2::new(engine.x + nozzle_half, engine.y),
        BRIGHT_GREEN,
    );

    let com = world_to_screen(lander.body.pos, cam);
    gizmos.circle_2d(to_bevy(com), 2.0, DIM_GREEN);
}

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

pub fn update_hud(
    time: Res<Time>,
    mut fps_smooth: Local<Option<f32>>,
    game: Res<GameState>,
    mut hud: Query<(&HudLine, &mut Text2d), Without<StatusText>>,
    mut status: Query<
        (&mut Text2d, &mut Transform, &mut Visibility),
        (With<StatusText>, Without<StatusPanel>),
    >,
    mut status_panel: Query<
        (&mut Transform, &mut Visibility),
        (With<StatusPanel>, Without<StatusText>),
    >,
) {
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

    for (HudLine(i), mut text) in &mut hud {
        if let Some(line) = lines.get(*i) {
            text.0 = line.clone();
        }
    }

    let status_y = status_y_below_horizon(&game);

    if let Ok((mut text, mut transform, mut vis)) = status.get_single_mut() {
        transform.translation.y = status_y;

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
        transform.translation.y = status_y;
        *vis = if game.status == GameStatus::Flying {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}
