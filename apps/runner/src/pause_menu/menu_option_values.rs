//! The Options screen: its rows, their current values (from the live config) and
//! how Left / Right / Confirm change them. Value rows apply live (the loop gets a
//! `Set*` action); action rows (save / reset) run on Confirm only.

use config_layers::{
    ResolvedConfig, VideoScaling, BUNDLED_PRESETS, MAX_RUN_AHEAD_FRAMES, SHADER_NONE,
};

use super::menu_types::{cycle, MenuAction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionItem {
    Shader,
    Scaling,
    RunAhead,
    Rewind,
    SaveForGame,
    SaveAsDefault,
    ResetGameSettings,
}

pub const OPTION_ITEMS: [OptionItem; 7] = [
    OptionItem::Shader,
    OptionItem::Scaling,
    OptionItem::RunAhead,
    OptionItem::Rewind,
    OptionItem::SaveForGame,
    OptionItem::SaveAsDefault,
    OptionItem::ResetGameSettings,
];

const SCALING_MODES: [VideoScaling; 3] = [
    VideoScaling::Aspect,
    VideoScaling::Integer,
    VideoScaling::Stretch,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionValues {
    /// `video.shader` choices: `none`, the bundled presets, then the user's custom
    /// preset path when one is configured.
    pub shaders: Vec<String>,
    /// Index into `shaders`.
    pub shader: usize,
    pub scaling: VideoScaling,
    pub run_ahead: u8,
    /// Why run-ahead cannot be changed this session (row shown disabled).
    pub run_ahead_unavailable: Option<&'static str>,
    pub rewind: bool,
    /// Why rewind cannot be used with this core (row shown disabled).
    pub rewind_unavailable: Option<&'static str>,
}

impl OptionValues {
    pub fn from_config(
        config: &ResolvedConfig,
        run_ahead_unavailable: Option<&'static str>,
        rewind_unavailable: Option<&'static str>,
    ) -> Self {
        let mut shaders: Vec<String> = std::iter::once(SHADER_NONE)
            .chain(BUNDLED_PRESETS.iter().copied())
            .map(str::to_string)
            .collect();
        let current = &config.video.shader;
        let shader = shaders
            .iter()
            .position(|s| s == current)
            .unwrap_or_else(|| {
                shaders.push(current.clone()); // custom `.slangp` path
                shaders.len() - 1
            });
        Self {
            shaders,
            shader,
            scaling: config.video.scaling,
            run_ahead: config.latency.run_ahead_frames,
            run_ahead_unavailable,
            rewind: config.rewind.enabled,
            rewind_unavailable,
        }
    }

    /// Why `item`'s row is disabled this session (`None`: usable).
    pub fn unavailable_reason(&self, item: OptionItem) -> Option<&'static str> {
        match item {
            OptionItem::RunAhead => self.run_ahead_unavailable,
            OptionItem::Rewind => self.rewind_unavailable,
            _ => None,
        }
    }

    /// Selected `video.shader` value.
    pub fn shader_value(&self) -> &str {
        self.shaders
            .get(self.shader)
            .map_or(SHADER_NONE, String::as_str)
    }

    /// Left (`forward = false`), Right or Confirm (`forward = true`) on `item`.
    pub fn adjust(&mut self, item: OptionItem, forward: bool, confirm: bool) -> Option<MenuAction> {
        if self.unavailable_reason(item).is_some() {
            return None;
        }
        match item {
            OptionItem::Shader => {
                self.shader = cycle(self.shader, self.shaders.len(), forward);
                Some(MenuAction::SetShader(self.shader_value().to_string()))
            }
            OptionItem::Scaling => {
                let index = SCALING_MODES.iter().position(|m| *m == self.scaling);
                self.scaling =
                    SCALING_MODES[cycle(index.unwrap_or(0), SCALING_MODES.len(), forward)];
                Some(MenuAction::SetScaling(self.scaling))
            }
            OptionItem::RunAhead => {
                let count = usize::from(MAX_RUN_AHEAD_FRAMES) + 1;
                self.run_ahead = cycle(usize::from(self.run_ahead), count, forward) as u8;
                Some(MenuAction::SetRunAhead(self.run_ahead))
            }
            OptionItem::Rewind => {
                self.rewind = !self.rewind;
                Some(MenuAction::SetRewind(self.rewind))
            }
            _ if !confirm => None,
            OptionItem::SaveForGame => Some(MenuAction::SaveForGame),
            OptionItem::SaveAsDefault => Some(MenuAction::SaveAsDefault),
            OptionItem::ResetGameSettings => Some(MenuAction::ResetGameSettings),
        }
    }
}
