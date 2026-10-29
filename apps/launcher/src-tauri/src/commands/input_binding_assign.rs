//! "Assign input X to action Y" with steal, computed in Rust so the UI can never
//! create a conflict: the input is appended to the target and removed from every
//! other action / hotkey of the same device, judged on the scope's **resolved**
//! binds (a game override keeps the holder's remaining inherited binds). The
//! result is one batch of layer writes (`set_layer_values`).

use config_layers::{
    Binds, InputConfig, Value, GAMEPAD_INPUT_NAMES, HOTKEYS, HOTKEYS_GAMEPAD_SECTION,
    HOTKEYS_KEYBOARD_SECTION, INPUT_GAMEPAD_SECTION, INPUT_KEYBOARD_SECTION, KEYBOARD_KEY_NAMES,
    MAX_BINDS_PER_ACTION, PAD_ACTIONS, RESERVED_GAMEPAD_INPUTS, RESERVED_KEYBOARD_KEYS,
    RESERVED_PAD_HOTKEY_INPUTS,
};
use serde::Deserialize;

use crate::command_error::{CommandError, CommandResult};

/// Which device's sections a binding lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputDevice {
    Keyboard,
    Gamepad,
}

/// Layer writes (`dotted key`, new binds) plus the dotted keys the input was
/// taken from (UI toast "Z removed from Y").
#[derive(Debug, PartialEq)]
pub struct AssignPlan {
    pub writes: Vec<(String, Option<Value>)>,
    pub removed_from: Vec<String>,
}

/// Plans binding `input` to `target` (`"input_keyboard.a"`, `"hotkeys_gamepad.save_state"`)
/// on `device`. `invalid` for a target outside the device's sections, an unknown or
/// reserved input (Escape, Guide, Back/Start as pad hotkey) or a full target.
pub fn plan_assign(
    resolved: &InputConfig,
    device: InputDevice,
    target: &str,
    input: &str,
) -> CommandResult<AssignPlan> {
    let sections = device_sections(resolved, device);
    let Some((section, key)) = target.split_once('.') else {
        return Err(unknown_target(target));
    };
    let Some((_, _, target_binds)) = sections
        .iter()
        .find(|(name, keys, _)| *name == section && keys.contains(&key))
    else {
        return Err(unknown_target(target));
    };
    check_input(device, section, input)?;
    let mut current = target_binds.get(key).cloned().unwrap_or_default();
    if !current.iter().any(|bound| bound == input) {
        if current.len() >= MAX_BINDS_PER_ACTION {
            return Err(CommandError::invalid(format!(
                "`{target}` already has {MAX_BINDS_PER_ACTION} inputs; remove one first."
            )));
        }
        current.push(input.to_string());
    }
    let mut writes = vec![(target.to_string(), Some(binds_value(&current)))];
    let mut removed_from = Vec::new();
    for (name, keys, binds) in &sections {
        for holder in keys.iter() {
            let dotted = format!("{name}.{holder}");
            let held = binds.get(*holder).cloned().unwrap_or_default();
            if dotted == target || !held.iter().any(|bound| bound == input) {
                continue;
            }
            let rest: Vec<String> = held.into_iter().filter(|bound| bound != input).collect();
            writes.push((dotted.clone(), Some(binds_value(&rest))));
            removed_from.push(dotted);
        }
    }
    Ok(AssignPlan {
        writes,
        removed_from,
    })
}

/// The device's action section and hotkey section with their keys and binds.
fn device_sections(
    resolved: &InputConfig,
    device: InputDevice,
) -> [(&'static str, &'static [&'static str], &Binds); 2] {
    match device {
        InputDevice::Keyboard => [
            (INPUT_KEYBOARD_SECTION, &PAD_ACTIONS, &resolved.keyboard),
            (
                HOTKEYS_KEYBOARD_SECTION,
                &HOTKEYS,
                &resolved.hotkeys_keyboard,
            ),
        ],
        InputDevice::Gamepad => [
            (INPUT_GAMEPAD_SECTION, &PAD_ACTIONS, &resolved.gamepad),
            (HOTKEYS_GAMEPAD_SECTION, &HOTKEYS, &resolved.hotkeys_gamepad),
        ],
    }
}

/// `invalid` unless `input` is a bindable name of `device` for `section`.
fn check_input(device: InputDevice, section: &str, input: &str) -> CommandResult<()> {
    let (names, reserved): (&[&str], &[&str]) = match device {
        InputDevice::Keyboard => (KEYBOARD_KEY_NAMES, &RESERVED_KEYBOARD_KEYS),
        InputDevice::Gamepad => (&GAMEPAD_INPUT_NAMES, &RESERVED_GAMEPAD_INPUTS),
    };
    if !names.contains(&input) {
        return Err(CommandError::invalid(format!(
            "`{input}` is not a supported input."
        )));
    }
    let pad_hotkey_reserved =
        section == HOTKEYS_GAMEPAD_SECTION && RESERVED_PAD_HOTKEY_INPUTS.contains(&input);
    if reserved.contains(&input) || pad_hotkey_reserved {
        return Err(CommandError::invalid(format!(
            "`{input}` is reserved and cannot be bound here."
        )));
    }
    Ok(())
}

fn unknown_target(target: &str) -> CommandError {
    CommandError::invalid(format!("Unknown control `{target}` for this device."))
}

fn binds_value(binds: &[String]) -> Value {
    Value::Array(binds.iter().map(|b| Value::from(b.as_str())).collect())
}

#[cfg(test)]
#[path = "input_binding_assign_tests.rs"]
mod tests;
