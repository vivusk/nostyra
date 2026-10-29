//! Serial rule table tests (DAT serial forms from the Redump Dreamcast DAT stats).

use super::*;

const DC: DiscSignature = DiscSignature::Dreamcast;
const PS: DiscSignature = DiscSignature::PlayStation;

#[test]
fn dreamcast_display_form() {
    assert_eq!(
        display_serial(DC, " mk-51064 ").as_deref(),
        Some("MK-51064")
    );
    assert_eq!(
        display_serial(DC, "T7001D  50").as_deref(),
        Some("T7001D-50")
    );
    assert_eq!(display_serial(DC, "HDR--0080").as_deref(), Some("HDR-0080"));
    assert_eq!(display_serial(DC, "   "), None);
}

#[test]
fn disc_and_dat_forms_share_a_key() {
    let shared = |disc: &str, dat: &str| {
        let disc = serial_lookup_keys(DC, disc);
        serial_lookup_keys(DC, dat).iter().any(|k| disc.contains(k))
    };
    assert!(shared("MK-51064", "51064"), "US DAT entries drop MK-");
    assert!(shared("MK-51064-50", "MK-51064-50"));
    assert!(
        !shared("MK-51064-50", "51064"),
        "EU disc is not the US entry"
    );
    assert!(shared("T-9708N", "T-9708N"));
    assert!(shared("HDR-0080", "HDR-0080"));
    assert!(shared("T7001D  50", "T-7001D-50"));
    assert!(shared("T-8101N", "T-8102N, T-8101N"), "comma lists");
    assert!(shared("T-9701N", "T-9701N-0"), "disc suffix");
    assert!(!shared("T-9701N", "T-9702N"));
}

#[test]
fn playstation_keys_are_unchanged() {
    assert_eq!(serial_lookup_keys(PS, "SLUS_012.34"), ["SLUS-01234"]);
    assert_eq!(
        display_serial(PS, "slus-01234").as_deref(),
        Some("SLUS-01234")
    );
    assert_eq!(serial_lookup_keys(PS, "SLUS-00001, SLUS-00002").len(), 2);
    assert!(serial_lookup_keys(PS, "junk").is_empty());
}

#[test]
fn psp_serial_forms() {
    assert_eq!(
        psp_normalize_serial(" ulus10041 ").as_deref(),
        Some("ULUS-10041")
    );
    assert_eq!(
        psp_normalize_serial("ULUS-10041").as_deref(),
        Some("ULUS-10041")
    );
    assert_eq!(
        psp_normalize_serial("HONEY-002").as_deref(),
        Some("HONEY-002")
    );
    assert_eq!(psp_normalize_serial("  "), None);
    assert_eq!(psp_key("ULES-01513/E").as_deref(), Some("ULES-01513"));
    assert_eq!(psp_key("npjh50001").as_deref(), Some("NPJH-50001"));
}

#[test]
fn gamecube_and_wii_ids_match_redump_serials_by_game_code() {
    const GC: DiscSignature = DiscSignature::GameCube;
    const WII: DiscSignature = DiscSignature::Wii;
    let shared = |signature, disc: &str, dat: &str| {
        let disc = serial_lookup_keys(signature, disc);
        serial_lookup_keys(signature, dat)
            .iter()
            .any(|k| disc.contains(k))
    };
    assert_eq!(display_serial(GC, " gale01 ").as_deref(), Some("GALE01"));
    assert_eq!(
        display_serial(WII, "RVL-RJ2E-USA").as_deref(),
        Some("RVL-RJ2E-USA")
    );
    assert_eq!(display_serial(GC, "  "), None);
    assert_eq!(serial_lookup_keys(GC, "GALE01"), ["GALE"]);
    assert!(shared(GC, "GALE01", "DL-DOL-GALE-USA"));
    assert!(shared(WII, "RJ2E01", "RVL-RJ2E-USA"));
    assert!(
        shared(GC, "GALE01", "DL-DOL-GALP-EUR, DL-DOL-GALE-USA-0"),
        "comma list + suffix"
    );
    assert!(shared(WII, "RJ2E01", "RVL-RJ2E-USA-1"));
    assert!(!shared(GC, "GALE01", "DL-DOL-GALP-EUR"), "other region");
    // Unknown forms give no rule key (the exact text still matches upstream).
    assert!(serial_lookup_keys(GC, "HOMEBREW-1").is_empty());
    assert!(serial_lookup_keys(WII, "RVL-RJ2-USA").is_empty());
    assert!(serial_lookup_keys(WII, "WAD1").is_empty());
}
