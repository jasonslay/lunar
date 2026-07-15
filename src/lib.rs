pub mod autopilot;
pub mod game;
pub mod input;
pub mod lander;
pub mod physics;
pub mod render;
pub mod world;

#[cfg(not(target_arch = "wasm32"))]
pub mod screenshot;

use bevy::prelude::*;
use bevy::window::WindowResolution;

use game::{GameState, update_game};
use input::{read_keyboard_input, ThrustInput};
use render::{draw_world, setup_camera, setup_hud, setup_lander, update_hud, update_lander_body};
use world::{SCREEN_HEIGHT, SCREEN_WIDTH};

pub fn build_app() -> App {
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
    .add_systems(Startup, (setup_camera, setup_hud, setup_lander))
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

    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, screenshot::apply_scene);

    app
}

#[cfg(not(target_arch = "wasm32"))]
pub fn configure_native(app: &mut App) {
    screenshot::configure(app);
}
