//! Initial / resync state transfer on the real core (F1, F18): the host always
//! sends SRAM + RTC + a `Normal` full state; the guest checks lengths before it
//! copies anything (the lobby already refused a `SyncBegin` with other sizes, this is
//! the last guard), then loads the state.

use libretro_host::{CoreSession, SavestateContext};
use netplay_session::lobby_protocol::RefusalReason;
use netplay_session::sync_transfer::SyncPayload;

/// Host: what every guest receives at `Start` (and on resync).
pub(crate) fn host_payload(session: &mut CoreSession) -> Result<SyncPayload, String> {
    let sram = session
        .save_ram_mut()
        .map(|m| m.to_vec())
        .unwrap_or_default();
    let rtc = session.rtc_mut().map(|m| m.to_vec());
    let mut state = Vec::new();
    session
        .serialize_into(&mut state, SavestateContext::Normal)
        .map_err(|err| format!("cannot save the start state: {err}"))?;
    Ok(SyncPayload { sram, rtc, state })
}

/// Guest: the host's memory sizes must equal ours (`None` RTC on both sides is fine).
pub(crate) fn check_lengths(
    sram_mine: usize,
    rtc_mine: Option<usize>,
    payload: &SyncPayload,
) -> Result<(), RefusalReason> {
    if payload.sram.len() != sram_mine {
        return Err(RefusalReason::SramSize {
            host: payload.sram.len() as u32,
            guest: sram_mine as u32,
        });
    }
    if payload.rtc.as_ref().map(Vec::len) != rtc_mine {
        return Err(RefusalReason::RtcMismatch);
    }
    Ok(())
}

/// Guest: lengths checked, then SRAM / RTC copied and the host's state loaded.
pub(crate) fn guest_load(session: &mut CoreSession, payload: &SyncPayload) -> Result<(), String> {
    let sram_mine = session.save_ram_mut().map_or(0, |m| m.len());
    let rtc_mine = session.rtc_mut().map(|m| m.len());
    check_lengths(sram_mine, rtc_mine, payload).map_err(|reason| reason.text())?;
    if let Some(sram) = session.save_ram_mut() {
        sram.copy_from_slice(&payload.sram);
    }
    if let (Some(rtc), Some(bytes)) = (session.rtc_mut(), payload.rtc.as_ref()) {
        rtc.copy_from_slice(bytes);
    }
    session
        .unserialize_from(&payload.state, SavestateContext::Normal)
        .map_err(|err| format!("cannot load the host's state: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(sram: usize, rtc: Option<usize>) -> SyncPayload {
        SyncPayload {
            sram: vec![1; sram],
            rtc: rtc.map(|n| vec![2; n]),
            state: vec![3; 4],
        }
    }

    #[test]
    fn guest_length_checks() {
        assert_eq!(check_lengths(8, None, &payload(8, None)), Ok(()));
        assert_eq!(check_lengths(8, Some(16), &payload(8, Some(16))), Ok(()));
        assert_eq!(
            check_lengths(8, None, &payload(32, None)),
            Err(RefusalReason::SramSize { host: 32, guest: 8 })
        );
        assert_eq!(
            check_lengths(8, None, &payload(8, Some(16))),
            Err(RefusalReason::RtcMismatch),
            "RTC None vs Some"
        );
        assert_eq!(
            check_lengths(8, Some(16), &payload(8, Some(8))),
            Err(RefusalReason::RtcMismatch)
        );
    }
}
