//! M20 phase-00 spike (feature `netplay-spike`, default off): ggrs + matchbox against
//! real cores. Deleted or promoted in phase 04; findings in
//! `plans/261109-2112-m20-netplay-rollback/reports/spike-00-results.md`.

mod core_rollback_game;
mod frame_inputs_source;
mod loopback_ggrs_peers;
mod loopback_peer_session;
mod shared_ggrs_channel;
mod spike_config;

#[cfg(test)]
mod mem_link_fixture_tests;
#[cfg(test)]
mod multitap_probe_tests;
#[cfg(test)]
mod stale_packet_tests;
#[cfg(test)]
mod synctest_core_run_tests;

pub(crate) use loopback_ggrs_peers::run_from_args;
