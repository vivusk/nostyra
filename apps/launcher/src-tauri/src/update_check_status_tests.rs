//! "Update available", automatic-check skip order and the version override parsing.

use super::*;

fn release(version: &str, has_portable_zip: bool) -> CachedRelease {
    CachedRelease {
        version: version.into(),
        title: format!("Nostyra {version}"),
        url: format!("https://github.com/vivusk/nostyra/releases/tag/v{version}"),
        published_at: None,
        excerpt: String::new(),
        has_portable_zip,
    }
}

#[test]
fn update_available_needs_newer_version_and_portable_zip() {
    let current = Version::new(0, 4, 0);
    assert!(is_update_available(Some(&release("0.5.0", true)), &current));
    assert!(!is_update_available(
        Some(&release("0.5.0", false)),
        &current
    ));
    assert!(!is_update_available(
        Some(&release("0.4.0", true)),
        &current
    ));
    assert!(!is_update_available(
        Some(&release("0.3.9", true)),
        &current
    ));
    assert!(!is_update_available(Some(&release("junk", true)), &current));
    assert!(!is_update_available(None, &current));
}

#[test]
fn skip_reason_order_is_disabled_then_debug_then_not_due() {
    assert_eq!(automatic_skip_reason(false, true, false), Some("disabled"));
    assert_eq!(automatic_skip_reason(true, true, false), Some("debug"));
    assert_eq!(automatic_skip_reason(true, false, false), Some("not_due"));
    assert_eq!(automatic_skip_reason(true, false, true), None);
}

#[test]
fn version_override_accepts_leading_v_and_ignores_junk() {
    let pkg = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    assert_eq!(
        resolve_current_version(Some("0.3.0".into())),
        Version::new(0, 3, 0)
    );
    assert_eq!(
        resolve_current_version(Some(" v0.3.0 ".into())),
        Version::new(0, 3, 0)
    );
    assert_eq!(resolve_current_version(Some("0.3".into())), pkg);
    assert_eq!(resolve_current_version(Some("  ".into())), pkg);
    assert_eq!(resolve_current_version(None), pkg);
}
