//! Problem texts of single-ROM ZIP reading that name the supported systems, built
//! from the registry (`system_catalog::ZIP_ARCHIVE_SYSTEMS`) so a new ZIP system
//! never needs a text edit.

use system_catalog::{plays_from_zip_archive, ALL_SYSTEMS};

/// Short labels of the systems a single-ROM ZIP may hold ("NES, SNES, …"), in
/// registry order.
pub(crate) fn zip_system_labels() -> String {
    ALL_SYSTEMS
        .iter()
        .filter(|spec| plays_from_zip_archive(spec.id))
        .map(|spec| spec.short_label)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A member whose extension only systems outside the ZIP group use.
pub(crate) fn unsupported_system(member: &str) -> String {
    format!(
        "ZIP ROM system is not supported: {member} (supported: {})",
        zip_system_labels()
    )
}

/// No member is a ROM of a ZIP system.
pub(crate) fn no_supported_rom() -> String {
    format!(
        "ZIP contains no supported ROM (supported: {})",
        zip_system_labels()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_the_registry_zip_group() {
        assert_eq!(
            zip_system_labels(),
            "SNES, NES, GBA, MD, SMS, GG, SG, 2600, 7800, Lynx"
        );
        assert!(unsupported_system("a.iso")
            .ends_with("(supported: SNES, NES, GBA, MD, SMS, GG, SG, 2600, 7800, Lynx)"));
    }
}
