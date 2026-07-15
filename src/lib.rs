pub mod autopilot;
pub mod game;
pub mod input;
pub mod lander;
pub mod physics;
pub mod render;
pub mod world;

#[cfg(not(target_arch = "wasm32"))]
pub mod screenshot;

use bevy::app::PluginGroup;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::WindowResolution;

use game::{GameState, update_game};
use input::{read_keyboard_input, ThrustInput};
use render::{draw_world, setup_camera, setup_hud, setup_lander, update_hud, update_lander_body};
use world::{SCREEN_HEIGHT, SCREEN_WIDTH};

fn window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "Lunar Lander".into(),
            resolution: WindowResolution::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            ..default()
        }),
        ..default()
    }
}

fn default_plugins() -> impl PluginGroup {
    let plugins = DefaultPlugins
        .build()
        .disable::<LogPlugin>()
        .set(window_plugin());

    #[cfg(not(target_arch = "wasm32"))]
    {
        use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
        use bevy::render::RenderPlugin;

        return plugins.set(RenderPlugin {
            render_creation: RenderCreation::Automatic(WgpuSettings {
                backends: Some(Backends::VULKAN),
                ..default()
            }),
            ..default()
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        plugins
    }
}

pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(LogPlugin {
        filter: "wgpu=error,naga=warn,wgpu_hal=error,calloop=error".into(),
        ..default()
    })
    .add_plugins(default_plugins())
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
