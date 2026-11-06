//! What the web player shares with the desktop besides names: the rule that refuses
//! a web save state made by another EmulatorJS core or build (same wording as the
//! desktop's strict-version refusal), and the pending play-session record the
//! desktop imports from `sessions/pending/` (Q5: matched by content sha1).

use ipc_protocol::{
    state_incompatibility, PendingSessionRecord, StateSidecar, RTC_FILE_NAME, SRAM_FILE_NAME,
    WEB_SRAM_FILE_NAME,
};
use serde::Serialize;

use crate::naming_json::WEB_CORE_VERSION;

/// Display name in refusal reasons ("Made by another EmulatorJS version").
const WEB_CORE_LABEL: &str = "EmulatorJS";

/// Why the web player refuses a state with this sidecar JSON for `ejs_core`, or
/// `None` when it may load. An unreadable sidecar is not refused (the desktop lists
/// such slots as loadable too); another core or EmulatorJS build is.
pub fn web_state_refusal(sidecar_json: &str, ejs_core: &str) -> Option<String> {
    let sidecar: StateSidecar = serde_json::from_str(sidecar_json).ok()?;
    if !sidecar.core_name.is_empty() && sidecar.core_name != ejs_core {
        return Some(format!("Made by another core ({})", sidecar.core_name));
    }
    let why = state_incompatibility(&sidecar, true, Some(WEB_CORE_VERSION), None)?;
    Some(capitalized(&why.reason(WEB_CORE_LABEL)))
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or(String::new(), |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Names of the per-game files the web player shares with the desktop.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFileNames {
    pub sram: &'static str,
    pub web_sram: &'static str,
    pub rtc: &'static str,
}

pub const DATA_FILE_NAMES: DataFileNames = DataFileNames {
    sram: SRAM_FILE_NAME,
    web_sram: WEB_SRAM_FILE_NAME,
    rtc: RTC_FILE_NAME,
};

/// `sessions/pending/<started_ms>-web.json`'s name (runner files use the pid).
pub fn web_pending_session_file_name(started_unix_ms: u64) -> String {
    format!("{started_unix_ms}-web.json")
}

/// JSON of a web play session for the desktop's pending import (`exit_code` 0:
/// the web records only sessions that ended normally or by closing the player).
pub fn web_pending_session_json(
    rom_path: &str,
    started_at: i64,
    play_seconds: u64,
    content_sha1: &str,
) -> String {
    let record = PendingSessionRecord {
        rom_path: rom_path.to_string(),
        started_at,
        play_seconds,
        exit_code: 0,
        content_sha1: Some(content_sha1.to_string()),
    };
    serde_json::to_string(&record).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming_json::web_state_sidecar;

    #[test]
    fn web_states_of_this_core_and_build_load_others_are_refused() {
        let ours = web_state_sidecar("snes9x", 1, None, None, b"x");
        assert_eq!(web_state_refusal(&ours, "snes9x"), None);
        assert_eq!(
            web_state_refusal(&ours, "mgba").as_deref(),
            Some("Made by another core (snes9x)")
        );
        let older = ours.replace(WEB_CORE_VERSION, "emulatorjs-4.2.2");
        assert_eq!(
            web_state_refusal(&older, "snes9x").as_deref(),
            Some("Made by another EmulatorJS version")
        );
        assert_eq!(web_state_refusal("not json", "snes9x"), None);
    }

    #[test]
    fn pending_session_is_a_desktop_record_with_the_content_sha1() {
        let json = web_pending_session_json("web:ab12cd34/Game.sfc", 1_700_000_000, 75, "6B47");
        let record: PendingSessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.rom_path, "web:ab12cd34/Game.sfc");
        assert_eq!(
            (record.started_at, record.play_seconds, record.exit_code),
            (1_700_000_000, 75, 0)
        );
        assert_eq!(record.content_sha1.as_deref(), Some("6B47"));
        assert_eq!(
            web_pending_session_file_name(1_700_000_000_123),
            "1700000000123-web.json"
        );
    }
}
