//! Runner resolve path (layers → conflicts → bindings) per system: a PlayStation
//! session maps the left trigger to RetroPad L2 (id 12), as does N64 (Z, plus the
//! stick keys), an SNES session keeps it on rewind; pristine configs warn about
//! nothing.

use std::path::Path;

use config_layers::ConfigLayerPaths;
use input_mapping::{GamepadInput, InputBindings, JoypadButton, RunnerHotkey, StickDirection};
use system_catalog::{SystemSpec, NINTENDO_64, PSX, SNES};
use winit::keyboard::KeyCode;

use super::{CliOverrides, EffectiveConfig};

fn session_config(root: &Path, system: &'static SystemSpec, core: &str) -> EffectiveConfig {
    let layers = ConfigLayerPaths::new(root, system.id, core, Path::new("roms/Game.bin"), "");
    EffectiveConfig::from_layers(layers, CliOverrides::default(), system.pad_actions)
}

#[test]
fn left_trigger_is_l2_on_ps1_and_rewind_on_snes() {
    let temp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let lt = GamepadInput::LeftTrigger;

    let psx = session_config(temp.path(), &PSX, "pcsx-rearmed");
    let (bindings, warnings) =
        InputBindings::from_config(&psx.resolved().input, Some(psx.system_actions()));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        bindings.pad_mask(|input| input == lt),
        JoypadButton::L2.mask()
    );
    assert_eq!(JoypadButton::L2 as u8, 12);
    assert_eq!(bindings.pad_hotkey(lt), None);

    let snes = session_config(temp.path(), &SNES, "snes9x");
    let (bindings, warnings) =
        InputBindings::from_config(&snes.resolved().input, Some(snes.system_actions()));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(bindings.pad_mask(|input| input == lt), 0);
    assert_eq!(bindings.pad_hotkey(lt), Some(RunnerHotkey::RewindHold));
}

#[test]
fn n64_left_trigger_is_z_and_stick_keys_reach_the_core() {
    let temp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let lt = GamepadInput::LeftTrigger;
    let n64 = session_config(temp.path(), &NINTENDO_64, "mupen64plus-next");
    let (bindings, warnings) =
        InputBindings::from_config(&n64.resolved().input, Some(n64.system_actions()));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        bindings.pad_mask(|input| input == lt),
        JoypadButton::L2.mask()
    );
    assert_eq!(bindings.pad_hotkey(lt), None, "rewind yields LT to Z");
    assert_eq!(
        bindings.key_sticks(KeyCode::KeyL),
        StickDirection::LeftRight.mask()
    );
    assert_eq!(
        bindings.key_sticks(KeyCode::KeyT),
        StickDirection::RightUp.mask()
    );

    let snes = session_config(temp.path(), &SNES, "snes9x");
    let (bindings, _) =
        InputBindings::from_config(&snes.resolved().input, Some(snes.system_actions()));
    assert_eq!(bindings.key_sticks(KeyCode::KeyL), 0, "no sticks on SNES");
}
