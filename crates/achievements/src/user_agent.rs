//! The User-Agent RetroAchievements sees:
//! `Nostyra/<pkg ver> (<OS> <maj>.<min>) <library_stem>/<core ver> rcheevos/<ver>`.
//! Rebuilt per session (core differs); every clause sanitized to `[A-Za-z0-9._-]`;
//! never a RetroArch UA. The rcheevos clause comes from rcheevos itself (D2).

use std::sync::{Arc, OnceLock};

use crate::{AchievementClient, FakeTransport, RequestDispatch};

/// `rcheevos/<major>.<minor>[.<patch>]` as `rc_client_get_user_agent_clause` reports
/// it (`rcheevos/12.5` for v12.5.0). Computed once with a throwaway client.
pub fn rcheevos_user_agent_clause() -> &'static str {
    static CLAUSE: OnceLock<String> = OnceLock::new();
    CLAUSE.get_or_init(|| {
        AchievementClient::new(
            Arc::new(FakeTransport::default()),
            RequestDispatch::Inline,
            Box::new(|_, _| 0),
        )
        .map(|client| client.user_agent_clause())
        .unwrap_or_else(|_| "rcheevos/unknown".to_owned())
    })
}

/// Full UA for a session with core `library_stem` (e.g. `mesen_libretro`) at
/// `core_version` (the core's `library_version`).
pub fn user_agent(library_stem: &str, core_version: &str) -> String {
    format!(
        "Nostyra/{} ({}) {}/{} {}",
        env!("CARGO_PKG_VERSION"),
        os_clause(),
        sanitize(library_stem),
        sanitize(core_version),
        rcheevos_user_agent_clause()
    )
}

/// Keeps `[A-Za-z0-9._-]`, maps anything else to `_`, trims `_`; empty → `unknown`.
fn sanitize(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `Windows 10.0`, `Linux 6.5`, `macOS 14.4`; version omitted when unknown.
fn os_clause() -> String {
    let (name, version) = os_name_and_version();
    match version {
        Some((major, minor)) => format!("{name} {major}.{minor}"),
        None => name.to_owned(),
    }
}

#[cfg(windows)]
fn os_name_and_version() -> (&'static str, Option<(u32, u32)>) {
    ("Windows", crate::ffi::windows_version())
}

#[cfg(target_os = "linux")]
fn os_name_and_version() -> (&'static str, Option<(u32, u32)>) {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    ("Linux", major_minor(&release))
}

#[cfg(target_os = "macos")]
fn os_name_and_version() -> (&'static str, Option<(u32, u32)>) {
    let plist = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
        .unwrap_or_default();
    let version = plist
        .split("<key>ProductVersion</key>")
        .nth(1)
        .and_then(|rest| rest.split("<string>").nth(1))
        .and_then(|rest| rest.split("</string>").next())
        .and_then(major_minor);
    ("macOS", version)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn os_name_and_version() -> (&'static str, Option<(u32, u32)>) {
    (std::env::consts::OS, None)
}

/// `"6.5.0-27-generic"` → `(6, 5)`.
#[cfg_attr(windows, allow(dead_code))]
fn major_minor(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.trim().split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rcheevos_clause_comes_from_the_library() {
        assert_eq!(rcheevos_user_agent_clause(), "rcheevos/12.5");
    }

    #[test]
    fn user_agent_has_every_clause_sanitized() {
        let ua = user_agent("mesen_libretro", "0.9.9 (abc/def)");
        let expected_tail = " mesen_libretro/0.9.9__abc_def rcheevos/12.5";
        assert!(
            ua.starts_with(&format!("Nostyra/{} (", env!("CARGO_PKG_VERSION"))),
            "{ua}"
        );
        assert!(ua.ends_with(expected_tail), "{ua}");
        let os = &ua[ua.find('(').unwrap_or(0) + 1..ua.find(')').unwrap_or(0)];
        assert!(!os.is_empty() && !ua.contains("RetroArch"), "{ua}");
        #[cfg(windows)]
        assert!(os.starts_with("Windows ") && os.contains('.'), "{os}");
    }

    #[test]
    fn sanitize_and_version_parsing() {
        assert_eq!(sanitize("Genesis Plus GX"), "Genesis_Plus_GX");
        assert_eq!(sanitize("  "), "unknown");
        assert_eq!(sanitize("v1.2-rc3"), "v1.2-rc3");
        assert_eq!(major_minor("6.5.0-27-generic"), Some((6, 5)));
        assert_eq!(major_minor("14"), Some((14, 0)));
        assert_eq!(major_minor(""), None);
    }
}
