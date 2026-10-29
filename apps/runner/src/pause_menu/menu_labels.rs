//! Text shown by the pause menu: screen titles, row labels and option values.

use super::menu_option_values::{OptionItem, OptionValues, OPTION_ITEMS};
use super::menu_types::{RootItem, Screen, SlotMode};
use super::pause_menu_state::PauseMenuState;

pub(super) fn screen_title(screen: Screen) -> &'static str {
    match screen {
        Screen::Root | Screen::ConfirmQuit | Screen::ConfirmReset => "Paused",
        Screen::Slots(SlotMode::Save) | Screen::ConfirmOverwrite(_) => "Save state",
        Screen::Slots(SlotMode::Load) => "Load state",
        Screen::Options => "Options",
        Screen::Discs => "Disc",
        Screen::Achievements => "Achievements",
    }
}

pub(super) fn root_label(item: RootItem) -> &'static str {
    match item {
        RootItem::Resume => "Resume",
        RootItem::SaveState => "Save state",
        RootItem::LoadState => "Load state",
        RootItem::Disc => "Disc",
        RootItem::Achievements => "Achievements",
        RootItem::ResetCore => "Reset game",
        RootItem::Options => "Options",
        RootItem::Quit => "Quit",
    }
}

pub(super) fn option_label(item: OptionItem) -> &'static str {
    match item {
        OptionItem::Shader => "Shader",
        OptionItem::Scaling => "Scaling",
        OptionItem::RunAhead => "Run-ahead",
        OptionItem::Rewind => "Rewind",
        OptionItem::SaveForGame => "Save for this game",
        OptionItem::SaveAsDefault => "Save as default",
        OptionItem::ResetGameSettings => "Reset game settings",
    }
}

/// `< value >` for value rows (and whether the row is enabled).
pub(super) fn option_value(item: OptionItem, options: &OptionValues) -> (Option<String>, bool) {
    let value = match item {
        OptionItem::Shader => shader_label(options.shader_value()),
        OptionItem::Scaling => format!("{:?}", options.scaling),
        _ if options.unavailable_reason(item).is_some() => {
            return (Some("Unavailable".into()), false);
        }
        OptionItem::RunAhead if options.run_ahead == 0 => "Off".into(),
        OptionItem::RunAhead => options.run_ahead.to_string(),
        OptionItem::Rewind => (if options.rewind { "On" } else { "Off" }).into(),
        _ => return (None, true),
    };
    (Some(format!("< {value} >")), true)
}

/// Footer without a question: a result message, why the option row under the cursor
/// is disabled (run-ahead / rewind refused), or the key hint.
pub(super) fn footer_hint(state: &PauseMenuState) -> &str {
    let options = state.options();
    let reason = (state.screen() == Screen::Options)
        .then(|| options.unavailable_reason(OPTION_ITEMS[state.option_cursor()]))
        .flatten();
    state
        .message()
        .or(reason)
        .unwrap_or("Enter / A: select   Esc / B: back")
}

/// `none` → "None", `crt-easymode` → "CRT Easymode", a `.slangp` path → "Custom".
fn shader_label(value: &str) -> String {
    if value.contains(['/', '\\']) {
        return "Custom".into();
    }
    value
        .split('-')
        .map(|word| match word {
            // Kept in sync with the launcher's `shaderLabel` (SHADER_ACRONYMS).
            "crt" => "CRT".to_string(),
            "lcd" => "LCD".to_string(),
            _ => {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_ascii_uppercase().to_string() + chars.as_str()
                })
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_labels() {
        assert_eq!(shader_label("none"), "None");
        assert_eq!(shader_label("crt-easymode"), "CRT Easymode");
        assert_eq!(shader_label("crt-geom"), "CRT Geom");
        assert_eq!(shader_label("zfast-lcd"), "Zfast LCD");
        assert_eq!(shader_label(r"C:\shaders\my.slangp"), "Custom");
        assert_eq!(shader_label("a--b"), "A  B", "empty words never panic");
    }

    #[test]
    fn refused_rows_read_unavailable_and_are_disabled() {
        let config = config_layers::ResolvedConfig::default();
        let open = OptionValues::from_config(&config, None, None);
        assert_eq!(
            option_value(OptionItem::Rewind, &open),
            (Some("< Off >".into()), true)
        );
        let refused = OptionValues::from_config(&config, Some("a"), Some("b"));
        for item in [OptionItem::RunAhead, OptionItem::Rewind] {
            let value = option_value(item, &refused);
            assert_eq!(value, (Some("Unavailable".into()), false), "{item:?}");
        }
    }
}
