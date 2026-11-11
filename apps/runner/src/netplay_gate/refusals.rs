//! Status texts of actions refused during netplay (same shape as the hardcore
//! texts) and the role × action table.

use super::{GatedAction, NetplaySeat};

pub const REWIND_OFF: &str = "Rewind is off during netplay";
pub const FAST_FORWARD_OFF: &str = "Fast-forward is off during netplay";
pub const LOAD_STATE_OFF: &str = "Loading states is off during netplay";
pub const RESET_OFF: &str = "Reset is off during netplay";
pub const DISC_SWAP_OFF: &str = "Disc swap is off during netplay";
pub const RUN_AHEAD_OFF: &str = "Run-ahead is off during netplay";
pub const SAVE_HOST_ONLY: &str = "Only the host can save in netplay";

/// Refusal of `action` for `seat` (`Off`: nothing refused).
pub fn refusal(seat: NetplaySeat, action: GatedAction) -> Option<&'static str> {
    if seat == NetplaySeat::Off {
        return None;
    }
    Some(match action {
        GatedAction::Rewind => REWIND_OFF,
        GatedAction::FastForward => FAST_FORWARD_OFF,
        GatedAction::LoadState => LOAD_STATE_OFF,
        GatedAction::Reset => RESET_OFF,
        GatedAction::DiscSwap => DISC_SWAP_OFF,
        GatedAction::RunAhead => RUN_AHEAD_OFF,
        GatedAction::SaveState if seat == NetplaySeat::Host => return None,
        GatedAction::SaveState => SAVE_HOST_ONLY,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_table_role_by_action() {
        use GatedAction::*;
        let all = [
            Rewind,
            FastForward,
            LoadState,
            SaveState,
            Reset,
            DiscSwap,
            RunAhead,
        ];
        for action in all {
            assert_eq!(refusal(NetplaySeat::Off, action), None, "{action:?}");
        }
        for seat in [
            NetplaySeat::Host,
            NetplaySeat::Player,
            NetplaySeat::Spectator,
        ] {
            assert_eq!(refusal(seat, Rewind), Some(REWIND_OFF));
            assert_eq!(refusal(seat, FastForward), Some(FAST_FORWARD_OFF));
            assert_eq!(refusal(seat, LoadState), Some(LOAD_STATE_OFF));
            assert_eq!(refusal(seat, Reset), Some(RESET_OFF));
            assert_eq!(refusal(seat, DiscSwap), Some(DISC_SWAP_OFF));
            assert_eq!(refusal(seat, RunAhead), Some(RUN_AHEAD_OFF));
        }
        assert_eq!(refusal(NetplaySeat::Host, SaveState), None, "host saves");
        assert_eq!(
            refusal(NetplaySeat::Player, SaveState),
            Some(SAVE_HOST_ONLY)
        );
        assert_eq!(
            refusal(NetplaySeat::Spectator, SaveState),
            Some(SAVE_HOST_ONLY)
        );
    }
}
