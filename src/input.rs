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

pub fn read_keyboard_input(
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
}

pub fn thrust_from_input(input: &ThrustInput) -> (f32, f32, f32) {
    (input.main, input.tilt_left, input.tilt_right)
}
