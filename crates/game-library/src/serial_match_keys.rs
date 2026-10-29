//! Keys a disc serial is indexed and looked up under, in the DAT and metadat indexes.
//! Always the exact (trimmed) text first, so existing exact matches never change;
//! with a disc signature also that system's normalised keys (`disc_serial_rule`):
//! a Dreamcast `MK-51035` finds the US DAT entry `51035`, comma lists split.

use disc_image::serial_lookup_keys;
use system_catalog::{ContentProbe, DiscSignature, SystemSpec};

/// The serial rule of a system: its disc signature (none for cartridges).
pub(crate) fn serial_rule(spec: &SystemSpec) -> Option<DiscSignature> {
    match spec.probe {
        ContentProbe::Disc(signature) => Some(signature),
        _ => None,
    }
}

/// Exact text, then the rule's keys (deduplicated, order kept).
pub(crate) fn serial_keys(rule: Option<DiscSignature>, raw: &str) -> Vec<String> {
    let mut keys = vec![raw.trim().to_owned()];
    if let Some(signature) = rule {
        for key in serial_lookup_keys(signature, raw) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    keys.retain(|key| !key.is_empty());
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_text_first_then_the_rules_keys() {
        assert_eq!(serial_keys(None, " SLUS-01234 "), ["SLUS-01234"]);
        let dc = serial_keys(Some(DiscSignature::Dreamcast), "MK-51035");
        assert_eq!(dc, ["MK-51035", "51035"]);
        let list = serial_keys(Some(DiscSignature::Dreamcast), "T-1N, T-2N");
        assert!(list.contains(&"T1N".to_owned()) && list.contains(&"T2N".to_owned()));
        assert_eq!(
            serial_rule(&system_catalog::DREAMCAST),
            Some(DiscSignature::Dreamcast)
        );
        assert_eq!(serial_rule(&system_catalog::SNES), None);
    }

    #[test]
    fn psp_serials_get_the_dash_and_lose_a_suffix() {
        let rule = Some(DiscSignature::Psp);
        assert_eq!(serial_keys(rule, "ULUS10041"), ["ULUS10041", "ULUS-10041"]);
        assert_eq!(
            serial_keys(rule, "ULES-01513/E"),
            ["ULES-01513/E", "ULES-01513"]
        );
    }
}
