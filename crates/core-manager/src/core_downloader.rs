//! Network half of an install: blocking HTTPS GET of the buildbot zip with
//! progress, cancel and a streaming size cap, then the offline installer.
//! Blocking by design: the launcher runs it on `spawn_blocking`, so no async
//! HTTP stack is needed.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use http_download_util::copy_with_limit;

use crate::buildbot_core_catalog::current_platform_core_source;
use crate::core_assets_installer::{core_assets_url, install_core_with_assets};
use crate::{CoreInfo, CoreManagerError, InstalledCore, MAX_CORE_BYTES};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Generous whole-request bound so a stalled connection cannot hang the
/// first-run flow forever (archive is a few MB).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Downloads `core_id` for this OS/arch from the libretro buildbot and installs
/// it into `cores_dir`, with the core's system files when it has some
/// (`CoreSpec::system_assets`: a second archive, all or nothing).
///
/// `probe` runs on the downloaded `*.tmp` file (launcher: `nostyra-runner
/// --probe-core`, so a bad core never crashes the launcher). `progress`
/// receives `(bytes_done, bytes_total)` summed over both downloads (the bar never
/// restarts); the total is `None` when a server sends no `Content-Length`.
/// Setting `cancel` aborts at the next chunk. Any failure leaves no new core
/// file, no tmp file and no staging folder behind.
pub fn install_core(
    core_id: &str,
    cores_dir: &Path,
    probe: impl FnOnce(&Path) -> Result<CoreInfo, String>,
    mut progress: impl FnMut(u64, Option<u64>),
    cancel: &AtomicBool,
) -> Result<InstalledCore, CoreManagerError> {
    let source = current_platform_core_source(core_id)?;
    let assets = system_catalog::core(core_id).and_then(|core| core.system_assets);
    let Some(assets) = assets else {
        let archive = download_archive(&source.archive_url, progress, cancel)?;
        return install_core_with_assets(&archive, None, core_id, cores_dir, probe, cancel);
    };
    let assets_url = core_assets_url(&assets);
    let assets_total = head_length(&assets_url);
    let archive = download_archive(
        &source.archive_url,
        |done, total| progress(done, sum(total, assets_total)),
        cancel,
    )?;
    let core_bytes = archive.len() as u64;
    let assets_archive = download_archive(
        &assets_url,
        |done, total| progress(core_bytes + done, sum(Some(core_bytes), total)),
        cancel,
    )?;
    let assets = Some((&assets, assets_archive.as_slice()));
    install_core_with_assets(&archive, assets, core_id, cores_dir, probe, cancel)
}

/// `a + b` when both are known.
fn sum(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    Some(a? + b?)
}

/// `Content-Length` of `url` from a HEAD request (best effort: `None` on any error),
/// so the first download's progress total already includes the second one.
fn head_length(url: &str) -> Option<u64> {
    let response = agent().head(url).call().ok()?;
    let length = response.headers().get("content-length")?;
    length.to_str().ok()?.parse().ok()
}

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .https_only(true)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build(),
    )
}

/// Fetches `url` into memory, refusing non-HTTPS URLs and bodies larger than
/// [`MAX_CORE_BYTES`] (checked against `Content-Length` and while streaming).
fn download_archive(
    url: &str,
    mut progress: impl FnMut(u64, Option<u64>),
    cancel: &AtomicBool,
) -> Result<Vec<u8>, CoreManagerError> {
    if !url.starts_with("https://") {
        return Err(CoreManagerError::InsecureUrl(url.to_owned()));
    }
    tracing::info!(url, "downloading libretro core");
    let response = agent().get(url).call()?;
    let body = response.into_body();
    let total = body.content_length();
    if total.is_some_and(|length| length > MAX_CORE_BYTES) {
        return Err(CoreManagerError::TooLarge {
            limit_bytes: MAX_CORE_BYTES,
        });
    }
    progress(0, total);

    let capacity = usize::try_from(total.unwrap_or(0)).unwrap_or(0);
    let mut archive = Vec::with_capacity(capacity);
    let mut reader = body.into_reader();
    copy_with_limit(
        &mut reader,
        &mut archive,
        MAX_CORE_BYTES,
        cancel,
        |done| progress(done, total),
        "core download interrupted",
    )?;
    tracing::info!(bytes = archive.len(), "core archive downloaded");
    Ok(archive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_total_is_known_only_when_both_are() {
        assert_eq!(sum(Some(3), Some(4)), Some(7));
        assert_eq!(sum(Some(3), None), None);
    }

    #[test]
    fn refuses_plain_http_before_any_network_io() {
        let cancel = AtomicBool::new(false);
        let result = download_archive("http://buildbot.libretro.com/x.zip", |_, _| {}, &cancel);
        assert!(matches!(result, Err(CoreManagerError::InsecureUrl(_))));
    }
}
