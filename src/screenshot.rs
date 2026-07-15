use std::path::PathBuf;
use std::process;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};

use crate::game::GameState;

#[derive(Resource)]
struct ScreenshotJob {
    path: PathBuf,
    frame_target: u32,
    frame: u32,
    triggered: bool,
}

pub fn configure(app: &mut App) {
    let Ok(path) = std::env::var("LUNAR_SCREENSHOT") else {
        return;
    };

    let frame_target = std::env::var("LUNAR_SCREENSHOT_FRAME")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(180);

    app.insert_resource(ScreenshotJob {
        path: PathBuf::from(path),
        frame_target,
        frame: 0,
        triggered: false,
    })
    .add_systems(Update, capture_screenshot);
}

pub fn apply_scene(mut game: ResMut<GameState>) {
    match std::env::var("LUNAR_SCENE").as_deref() {
        Ok("autopilot") => {
            game.simulate_autopilot_until(|game| {
                let dx = (game.world.pad_center_x - game.lander.body.pos.x).abs();
                let alt = game.world.clearance_above_terrain(&game.lander.hull_world);
                dx < 28.0 && alt < 16.0 && alt > 5.0
            });
        }
        Ok("landed") => {
            *game = GameState::landed_demo(game.seed);
        }
        _ => {}
    }
}

fn capture_screenshot(mut commands: Commands, mut job: ResMut<ScreenshotJob>) {
    job.frame += 1;
    if job.triggered || job.frame < job.frame_target {
        return;
    }

    job.triggered = true;
    let path = job.path.to_string_lossy().into_owned();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(|_: Trigger<ScreenshotCaptured>| process::exit(0));
}
