//! Netplay menu (non-pausing, F5): the root shows Resume, the session line
//! ("Netplay · Host, 2 players, ping 40 ms"), Save state + Resync for the host, and
//! Quit. Load, reset, disc swap and options stay hidden (lockstep).

use super::super::menu_types::RootItem;
use super::PauseMenuState;

/// The session line and the seat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NetplayRows {
    pub info: String,
    pub host: bool,
}

/// Root rows of the netplay menu: no load / reset / disc / options (lockstep);
/// the host may save a state and resync.
pub fn netplay_root_items(host: bool) -> Vec<RootItem> {
    let mut items = vec![RootItem::Resume, RootItem::NetplayInfo];
    if host {
        items.extend([RootItem::SaveState, RootItem::Resync]);
    }
    items.push(RootItem::Quit);
    items
}

impl PauseMenuState {
    /// Turns the root into the netplay menu (`None`: the normal menu).
    pub fn set_netplay(&mut self, info: Option<(String, bool)>) {
        self.netplay = info.map(|(info, host)| NetplayRows { info, host });
    }

    /// Row label + trailing value + enabled of a root row.
    pub fn root_row(&self, item: RootItem) -> (String, Option<String>, bool) {
        let label = super::super::menu_labels::root_label(item);
        match (item, &self.netplay) {
            (RootItem::NetplayInfo, Some(rows)) => (label.into(), Some(rows.info.clone()), false),
            (RootItem::LoadState, _) => (label.into(), Some("›".into()), self.load_enabled()),
            (RootItem::Resume | RootItem::ResetCore | RootItem::Quit | RootItem::Resync, _) => {
                (label.into(), None, true)
            }
            _ => (label.into(), Some("›".into()), true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::menu_option_values::OptionValues;
    use super::super::super::menu_types::{MenuAction, NavEvent};
    use super::*;

    fn menu(host: bool) -> PauseMenuState {
        let config = config_layers::ResolvedConfig::default();
        let options = OptionValues::from_config(&config, None, None);
        let mut state = PauseMenuState::new(Vec::new(), options, 0);
        state.set_netplay(Some(("Host, 2 players".into(), host)));
        state
    }

    #[test]
    fn host_menu_has_resync_and_no_copy_code_or_load_rows() {
        let host = menu(true);
        let items = host.root_items();
        assert_eq!(
            items,
            [
                RootItem::Resume,
                RootItem::NetplayInfo,
                RootItem::SaveState,
                RootItem::Resync,
                RootItem::Quit
            ]
        );
        let labels: Vec<String> = items.iter().map(|i| host.root_row(*i).0).collect();
        assert!(
            !labels.iter().any(|l| l.to_lowercase().contains("code")),
            "{labels:?}"
        );
        let info = host.root_row(RootItem::NetplayInfo);
        assert_eq!(
            (info.1.as_deref(), info.2),
            (Some("Host, 2 players"), false)
        );
        let guest = menu(false).root_items();
        assert_eq!(
            guest,
            [RootItem::Resume, RootItem::NetplayInfo, RootItem::Quit]
        );
    }

    #[test]
    fn resync_row_asks_the_loop() {
        let mut host = menu(true);
        for _ in 0..3 {
            host.handle(NavEvent::Down);
        }
        assert_eq!(host.handle(NavEvent::Confirm), Some(MenuAction::Resync));
        let mut guest = menu(false);
        guest.handle(NavEvent::Down);
        assert_eq!(
            guest.handle(NavEvent::Confirm),
            None,
            "info row does nothing"
        );
    }
}
