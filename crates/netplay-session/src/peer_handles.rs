//! ggrs handles of a session: host = handle 0 (the `Welcome` sender, F31), then the
//! players in join order, then spectators from `num_players` up. The host assigns
//! them and sends the result in `Start`; every peer rebuilds the same map from it
//! (matchbox `players()` would order by `PeerId` instead, spike 00).

use crate::{NetplayError, PeerKey};

/// `PeerKey ↔ handle` of one ggrs session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerHandles {
    players: Vec<(PeerKey, usize)>,
    spectators: Vec<(PeerKey, usize)>,
}

impl PeerHandles {
    /// Host side: host first, `players` and `spectators` in join order.
    pub fn assign(host: PeerKey, players: &[PeerKey], spectators: &[PeerKey]) -> Self {
        let players: Vec<(PeerKey, usize)> = std::iter::once(host)
            .chain(players.iter().copied().filter(|&p| p != host))
            .enumerate()
            .map(|(handle, peer)| (peer, handle))
            .collect();
        let first = players.len();
        let spectators = spectators
            .iter()
            .enumerate()
            .map(|(i, &peer)| (peer, first + i))
            .collect();
        Self {
            players,
            spectators,
        }
    }

    /// Guest side: the `Start` lists, checked (players 0..n, spectators n.., unique).
    pub fn from_start(
        players: &[(PeerKey, usize)],
        spectators: &[(PeerKey, usize)],
    ) -> Result<Self, NetplayError> {
        let n = players.len();
        let players_ok = players.iter().enumerate().all(|(i, &(_, h))| h == i);
        let spectators_ok = spectators.iter().enumerate().all(|(i, &(_, h))| h == n + i);
        let mut peers: Vec<PeerKey> = players.iter().chain(spectators).map(|p| p.0).collect();
        peers.sort();
        peers.dedup();
        if n == 0 || !players_ok || !spectators_ok || peers.len() != n + spectators.len() {
            return Err(NetplayError::Protocol("bad handle list in Start".into()));
        }
        Ok(Self {
            players: players.to_vec(),
            spectators: spectators.to_vec(),
        })
    }

    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    /// The host (handle 0).
    pub fn host(&self) -> PeerKey {
        self.players[0].0
    }

    pub fn players(&self) -> &[(PeerKey, usize)] {
        &self.players
    }

    pub fn spectators(&self) -> &[(PeerKey, usize)] {
        &self.spectators
    }

    pub fn handle_of(&self, peer: PeerKey) -> Option<usize> {
        let all = self.players.iter().chain(&self.spectators);
        all.into_iter().find(|p| p.0 == peer).map(|p| p.1)
    }

    pub fn peer_of(&self, handle: usize) -> Option<PeerKey> {
        let all = self.players.iter().chain(&self.spectators);
        all.into_iter().find(|p| p.1 == handle).map(|p| p.0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn host_first_then_join_order_then_spectators() {
        let (host, a, b, s1, s2) = (PeerKey(50), PeerKey(9), PeerKey(70), PeerKey(1), PeerKey(2));
        let handles = PeerHandles::assign(host, &[b, a], &[s2, s1]);
        assert_eq!(handles.players(), &[(host, 0), (b, 1), (a, 2)]);
        assert_eq!(handles.spectators(), &[(s2, 3), (s1, 4)]);
        assert_eq!(handles.handle_of(a), Some(2));
        assert_eq!(handles.peer_of(4), Some(s1));
        assert_eq!(handles.host(), host);
        assert_eq!(handles.num_players(), 3);
        assert_eq!(handles.handle_of(PeerKey(3)), None);
        let back = PeerHandles::from_start(handles.players(), handles.spectators()).unwrap();
        assert_eq!(back, handles);
    }

    #[test]
    fn bad_start_lists_are_refused() {
        let (a, b) = (PeerKey(1), PeerKey(2));
        assert!(PeerHandles::from_start(&[], &[]).is_err());
        assert!(PeerHandles::from_start(&[(a, 1)], &[]).is_err());
        assert!(PeerHandles::from_start(&[(a, 0)], &[(b, 0)]).is_err());
        assert!(PeerHandles::from_start(&[(a, 0), (a, 1)], &[]).is_err());
        assert!(PeerHandles::from_start(&[(a, 0)], &[(b, 1)]).is_ok());
    }
}
