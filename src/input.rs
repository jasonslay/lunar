use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct ThrustInput {
    pub main: f32,
    pub tilt_left: f32,
    pub tilt_right: f32,
    pub toggle_autopilot: bool,
    pub reset: bool,
    pub new_level: bool,
}

pub fn read_player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input: ResMut<ThrustInput>,
) {
    input.main = if keyboard.any_pressed([
        KeyCode::ArrowUp,
        KeyCode::KeyW,
        KeyCode::Space,
    ]) {
        1.0
    } else {
        0.0
    };

    input.tilt_left = if keyboard.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
        1.0
    } else {
        0.0
    };

    input.tilt_right = if keyboard.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
        1.0
    } else {
        0.0
    };

    input.toggle_autopilot = keyboard.just_pressed(KeyCode::KeyP);
    input.reset = keyboard.just_pressed(KeyCode::KeyR);
    input.new_level = keyboard.just_pressed(KeyCode::KeyN);

    #[cfg(target_arch = "wasm32")]
    merge_touch_input(&mut input);
}

#[cfg(target_arch = "wasm32")]
fn merge_touch_input(input: &mut ThrustInput) {
    use js_sys::{Object, Reflect};
    use wasm_bindgen::{JsCast, JsValue};

    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(touch) = Reflect::get(&window, &JsValue::from_str("__lunarTouch")) else {
        return;
    };
    if touch.is_undefined() {
        return;
    }
    let Some(obj) = touch.dyn_ref::<Object>() else {
        return;
    };

    let read_f32 = |key: &str| -> f32 {
        Reflect::get(obj, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.0)
    };
    let read_bool = |key: &str| -> bool {
        Reflect::get(obj, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };

    input.main = input.main.max(read_f32("main"));
    input.tilt_left = input.tilt_left.max(read_f32("tilt_left"));
    input.tilt_right = input.tilt_right.max(read_f32("tilt_right"));
    input.toggle_autopilot |= read_bool("toggle_autopilot");
    input.reset |= read_bool("reset");
    input.new_level |= read_bool("new_level");

    let _ = Reflect::set(obj, &JsValue::from_str("toggle_autopilot"), &JsValue::from(false));
    let _ = Reflect::set(obj, &JsValue::from_str("reset"), &JsValue::from(false));
    let _ = Reflect::set(obj, &JsValue::from_str("new_level"), &JsValue::from(false));
}

pub fn thrust_from_input(input: &ThrustInput) -> (f32, f32, f32) {
    (input.main, input.tilt_left, input.tilt_right)
}
