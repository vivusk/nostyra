//! The game window title: ROM name, soft-patch label, active slot and the latest
//! status (state hotkeys, pause, fast-forward).

use super::GameSaveData;

impl GameSaveData {
    /// e.g. `Nostyra — Chrono Trigger [IPS patch] · Slot 3 · Saved`.
    pub fn window_title(&self, status: &str) -> String {
        let label = self
            .title_label
            .as_ref()
            .map_or(String::new(), |label| format!(" [{label}]"));
        let base = format!("Nostyra — {}{label} · Slot {}", self.rom_name, self.slot);
        if status.is_empty() {
            base
        } else {
            format!("{base} · {status}")
        }
    }
}
