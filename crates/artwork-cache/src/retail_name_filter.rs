//! Retail-release filter for No-Intro names, used by the boxart coverage
//! check: prototypes, demos, pirate and homebrew dumps rarely have box art, so
//! the exit criterion (≥ 90 % covers) is measured on retail names only. Same
//! exclusion list as the research measurement.

/// Tag words that mark a non-retail dump when a `(…)` tag contains them:
/// `(Proto 2)`, `(Possible Proto)`, `(Demo 1)`, `(Auto Demo)`, `(Beta)`,
/// `(Taikenban Sample ROM)`.
const NON_RETAIL_TAG_WORDS: &[&str] = &["Proto", "Demo", "Beta", "Sample"];

/// Whole tags (or comma-separated tag parts) that mark a non-retail dump.
const NON_RETAIL_TAGS: &[&str] = &["Pirate", "Unl", "Aftermarket"];

/// True unless one of the name's `(…)` tags marks a prototype, demo, beta,
/// sample, pirate, unlicensed or aftermarket (homebrew) release.
pub fn is_retail_name(name: &str) -> bool {
    !parenthesised_tags(name).any(|tag| {
        tag.split(',').map(str::trim).any(|part| {
            NON_RETAIL_TAGS.contains(&part)
                || part
                    .split_whitespace()
                    .any(|word| NON_RETAIL_TAG_WORDS.iter().any(|w| word.starts_with(w)))
        })
    })
}

/// Contents of every `(…)` group in `name` (title words outside parentheses,
/// like "Demolition Man", are never inspected).
fn parenthesised_tags(name: &str) -> impl Iterator<Item = &str> {
    name.split('(')
        .skip(1)
        .filter_map(|chunk| chunk.split_once(')').map(|(tag, _)| tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_names_pass() {
        assert!(is_retail_name("Chrono Trigger (USA)"));
        assert!(is_retail_name("Demolition Man (USA)"));
        assert!(is_retail_name("Super Mario World (USA) (Rev 1)"));
        assert!(is_retail_name("Tetris Attack (Europe) (En,Ja)"));
    }

    #[test]
    fn non_retail_names_fail() {
        for name in [
            "X (USA) (Proto)",
            "X (Japan) (Proto 2)",
            "X (USA) (Possible Proto)",
            "X (USA) (Demo 1)",
            "X (Japan) (Auto Demo)",
            "X (USA) (Beta)",
            "101 Dalmatas (USA) (Pirate)",
            "Chrono Trigger (Japan) (Sample)",
            "Chrono Trigger (Japan) (Taikenban Sample ROM)",
            "X (World) (Unl)",
            "X (World) (Aftermarket) (Unl)",
        ] {
            assert!(!is_retail_name(name), "{name}");
        }
    }
}
