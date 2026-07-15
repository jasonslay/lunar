mod autopilot;
mod game;
mod input;
mod lander;
mod physics;
mod render;
mod screenshot;
mod world;

use bevy::prelude::*;
use bevy::window::WindowResolution;

use game::{GameState, update_game};
use input::{read_keyboard_input, ThrustInput};
use render::{draw_world, setup_camera, setup_hud, setup_lander, update_hud, update_lander_body};
use screenshot::{apply_scene, configure as configure_screenshot};
use world::{SCREEN_HEIGHT, SCREEN_WIDTH};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Lunar Lander".into(),
            resolution: WindowResolution::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            ..default()
        }),
        ..default()
    }))
    .init_resource::<ThrustInput>()
    .insert_resource(GameState::new(42))
    .add_systems(Startup, (setup_camera, setup_hud, setup_lander, apply_scene))
    .add_systems(
        Update,
        (
            read_keyboard_input,
            update_game,
            update_lander_body,
            draw_world,
            update_hud,
        )
            .chain(),
    );

    configure_screenshot(&mut app);
    app.run();
}
