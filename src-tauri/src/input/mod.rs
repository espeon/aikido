use gilrs::{Button, EventType, Gilrs};
use serde::Serialize;
use specta::Type;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Type)]
pub struct GamepadEvent {
    pub kind: String,
    pub button: String,
    pub value: f32,
    pub gamepad_id: u32,
}

const STICK_DEADZONE: f32 = 0.2;
const DPAD_INITIAL_DELAY: Duration = Duration::from_millis(200);
const DPAD_REPEAT_DELAY: Duration = Duration::from_millis(80);

pub fn start_gamepad_bridge(app: AppHandle) {
    std::thread::spawn(move || {
        let mut gilrs = match Gilrs::new() {
            Ok(g) => {
                tracing::info!("gilrs initialized, gamepads: {}", g.gamepads().count());
                g
            }
            Err(e) => {
                tracing::error!("gilrs init failed: {}", e);
                return;
            }
        };

        let mut stick_state: Option<(u32, String, Instant, bool)> = None;

        loop {
            let event = gilrs.next_event_blocking(None);
            if let Some(evt) = event {
                tracing::debug!("gamepad event: {:?}", evt);
            let gamepad_id = gamepad_id_to_u32(evt.id);

            if matches!(evt.event, EventType::Connected) {
                if let Some(gamepad) = gilrs.connected_gamepad(evt.id) {
                    tracing::info!("controller connected: {}", gamepad.name());
                }
            }

            match evt.event {
                    EventType::ButtonPressed(button, _) => {
                        let name = button_name(button);
                        tracing::info!("gamepad button: {} pressed", name);
                        let _ = app.emit(
                            "gamepad",
                            GamepadEvent {
                                kind: "ButtonPressed".into(),
                                button: name,
                                value: 1.0,
                                gamepad_id,
                            },
                        );
                    }
                    EventType::ButtonReleased(button, _) => {
                        let name = button_name(button);
                        let _ = app.emit(
                            "gamepad",
                            GamepadEvent {
                                kind: "ButtonReleased".into(),
                                button: name,
                                value: 0.0,
                                gamepad_id,
                            },
                        );
                    }
                    EventType::AxisChanged(axis, value, _) => {
                        let name = axis_name(axis);
                        let deadzoned = if value.abs() < STICK_DEADZONE {
                            0.0
                        } else {
                            value
                        };

                        let _ = app.emit(
                            "gamepad",
                            GamepadEvent {
                                kind: "AxisChanged".into(),
                                button: name.clone(),
                                value: deadzoned,
                                gamepad_id,
                            },
                        );

                        if name.starts_with("LStick") || name.starts_with("RStick") {
                            let dpad_button = stick_to_dpad(&name, deadzoned);
                            handle_stick_dpad(
                                &mut stick_state,
                                gamepad_id,
                                dpad_button,
                                &app,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    });
}

fn handle_stick_dpad(
    state: &mut Option<(u32, String, Instant, bool)>,
    gamepad_id: u32,
    current_button: Option<String>,
    app: &AppHandle,
) {
    let now = Instant::now();

    match (state.as_mut(), current_button) {
        (Some((_, last_button, last_time, started)), Some(new_button))
            if *last_button == new_button =>
        {
            if *started {
                if now - *last_time >= DPAD_REPEAT_DELAY {
                    *last_time = now;
                    emit_dpad(app, gamepad_id, &new_button, true);
                }
            } else if now - *last_time >= DPAD_INITIAL_DELAY {
                *started = true;
                *last_time = now;
                emit_dpad(app, gamepad_id, &new_button, true);
            }
        }
        (Some(_), Some(new_button)) => {
            let button_clone = new_button.clone();
            if let Some((_, old_button, _, _)) = state.replace((gamepad_id, new_button, now, false))
            {
                emit_dpad(app, gamepad_id, &old_button, false);
            }
            emit_dpad(app, gamepad_id, &button_clone, true);
        }
        (Some(state_data), None) => {
            let (_, old_button, _, _) = state_data;
            emit_dpad(app, gamepad_id, old_button, false);
            *state = None;
        }
        (None, Some(new_button)) => {
            *state = Some((gamepad_id, new_button.clone(), now, false));
            emit_dpad(app, gamepad_id, &new_button, true);
        }
        (None, None) => {}
    }
}

fn emit_dpad(app: &AppHandle, gamepad_id: u32, button: &str, pressed: bool) {
    let _ = app.emit(
        "gamepad",
        GamepadEvent {
            kind: if pressed {
                "ButtonPressed".into()
            } else {
                "ButtonReleased".into()
            },
            button: button.to_string(),
            value: if pressed { 1.0 } else { 0.0 },
            gamepad_id,
        },
    );
}

fn stick_to_dpad(axis: &str, value: f32) -> Option<String> {
    if value == 0.0 {
        return None;
    }
    let dir = if value > 0.0 { "Up" } else { "Down" };
    let (stick, axis_name) = if axis.starts_with("LStick") {
        ("LStick", &axis["LStick".len()..])
    } else {
        ("RStick", &axis["RStick".len()..])
    };
    if axis_name.contains('Y') {
        Some(format!("{}{}", stick, dir))
    } else if axis_name.contains('X') {
        Some(format!("{}{}", stick, if value > 0.0 { "Right" } else { "Left" }))
    } else {
        None
    }
}

fn button_name(button: Button) -> String {
    match button {
        Button::South => "A".into(),
        Button::East => "B".into(),
        Button::North => "X".into(),
        Button::West => "Y".into(),
        Button::LeftTrigger => "LB".into(),
        Button::RightTrigger => "RB".into(),
        Button::LeftTrigger2 => "LT".into(),
        Button::RightTrigger2 => "RT".into(),
        Button::Start => "Start".into(),
        Button::Select => "Select".into(),
        Button::LeftThumb => "LStickClick".into(),
        Button::RightThumb => "RStickClick".into(),
        Button::DPadUp => "DPadUp".into(),
        Button::DPadDown => "DPadDown".into(),
        Button::DPadLeft => "DPadLeft".into(),
        Button::DPadRight => "DPadRight".into(),
        _ => format!("{:?}", button),
    }
}

fn axis_name(axis: gilrs::Axis) -> String {
    match axis {
        gilrs::Axis::LeftStickX => "LStickX".into(),
        gilrs::Axis::LeftStickY => "LStickY".into(),
        gilrs::Axis::RightStickX => "RStickX".into(),
        gilrs::Axis::RightStickY => "RStickY".into(),
        gilrs::Axis::LeftZ => "LT".into(),
        gilrs::Axis::RightZ => "RT".into(),
        _ => format!("{:?}", axis),
    }
}

fn gamepad_id_to_u32(id: gilrs::GamepadId) -> u32 {
    let val: usize = id.into();
    val as u32
}
