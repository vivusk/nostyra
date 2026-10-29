//! `--load-state` / `--headless-save-slot` values: `0..=9` or `auto` (the resume
//! point), spelled by `ipc_protocol::StateSlotId` so runner and launcher agree.

use ipc_protocol::StateSlotId;

/// clap value parser for slot arguments.
pub(super) fn parse_slot_id(value: &str) -> Result<StateSlotId, String> {
    value.parse()
}

/// Numbered slot active at start: the `--load-state` slot if numbered, else
/// `--state-slot`. Resuming `auto` starts on `--state-slot` (default 0), since F1 and
/// the slot grid never write the resume point.
pub(super) fn start_slot(load_state: Option<StateSlotId>, state_slot: u8) -> u8 {
    match load_state {
        Some(StateSlotId::Numbered(slot)) => slot,
        Some(StateSlotId::Auto) | None => state_slot,
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::runner_cli_args::RunnerCliArgs;

    fn parse(extra: &[&str]) -> Result<RunnerCliArgs, clap::Error> {
        let base = ["nostyra-runner", "--core", "c", "--rom", "r"];
        RunnerCliArgs::try_parse_from(base.iter().chain(extra))
    }

    #[test]
    fn load_state_sets_start_slot_and_conflicts_with_state_slot() {
        assert_eq!(parse(&["--load-state", "4"]).unwrap().start_slot(), 4);
        assert_eq!(parse(&["--state-slot", "2"]).unwrap().start_slot(), 2);
        assert!(parse(&["--load-state", "1", "--state-slot", "2"]).is_err());
        assert!(parse(&["--headless-save-slot", "0"]).is_err()); // needs --headless-frames
    }

    #[test]
    fn auto_is_accepted_and_starts_on_slot_zero() {
        let args = parse(&["--load-state", "auto"]).unwrap();
        assert_eq!(args.load_state, Some(StateSlotId::Auto));
        assert_eq!(args.start_slot(), 0);
        assert!(parse(&["--load-state", "auto", "--state-slot", "2"]).is_err());
        let headless = parse(&["--headless-frames", "1", "--headless-save-slot", "auto"]);
        assert_eq!(
            headless.unwrap().headless_save_slot,
            Some(StateSlotId::Auto)
        );
    }

    #[test]
    fn bad_slot_values_are_usage_errors() {
        for bad in ["10", "-1", "AUTO", "slot1", ""] {
            assert!(parse(&["--load-state", bad]).is_err(), "{bad}");
        }
        let save = |v: &str| parse(&["--headless-frames", "1", "--headless-save-slot", v]);
        assert!(save("10").is_err() && save("9").is_ok());
    }

    #[test]
    fn start_slot_rule() {
        assert_eq!(start_slot(None, 3), 3);
        assert_eq!(start_slot(Some(StateSlotId::Numbered(7)), 3), 7);
        assert_eq!(start_slot(Some(StateSlotId::Auto), 0), 0);
    }
}
