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
use input::{read_player_input, ThrustInput};
use render::{
    draw_world, setup_camera, setup_hud, setup_lander, update_hud, update_lander_body, WasmBuildId,
};
use world::{SCREEN_HEIGHT, SCREEN_WIDTH};

#[cfg(target_arch = "wasm32")]
fn load_wasm_build_id(mut commands: Commands) {
    use js_sys::Reflect;
    use wasm_bindgen::JsValue;

    let suffix = web_sys::window()
        .and_then(|window| Reflect::get(&window, &JsValue::from_str("__lunarWasmHash")).ok())
        .and_then(|value| value.as_string())
        .unwrap_or_default();
    commands.insert_resource(WasmBuildId(suffix));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_wasm_build_id(mut commands: Commands) {
    commands.insert_resource(WasmBuildId::default());
}

fn window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "Lunar Lander".into(),
            resolution: WindowResolution::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            #[cfg(target_arch = "wasm32")]
            canvas: Some("#bevy_canvas".into()),
            #[cfg(target_arch = "wasm32")]
            fit_canvas_to_parent: true,
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
    .add_systems(
        Startup,
        (load_wasm_build_id, setup_camera, setup_hud, setup_lander).chain(),
    )
    .add_systems(
        Update,
        (
            read_player_input,
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
