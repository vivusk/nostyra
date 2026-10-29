//! Table tests for No-Intro name → thumbnail file name candidates.

use artwork_cache::{fallback_names, resolve_names, sanitize_thumbnail_name};

#[test]
fn forbidden_characters_become_underscores() {
    assert_eq!(
        sanitize_thumbnail_name("Q*bert's Qubes (USA)"),
        "Q_bert's Qubes (USA)"
    );
    assert_eq!(
        sanitize_thumbnail_name(r#"A&B*C/D:E`F<G>H?I\J|K"L"#),
        "A_B_C_D_E_F_G_H_I_J_K_L"
    );
    // Apostrophe, comma, period, hyphen, parentheses and non-ASCII are kept.
    assert_eq!(
        sanitize_thumbnail_name("Pop'n TwinBee, Dr. Mario - Rev. (Japan)"),
        "Pop'n TwinBee, Dr. Mario - Rev. (Japan)"
    );
    assert_eq!(
        sanitize_thumbnail_name("Pokémon’s (Japan)"),
        "Pokémon’s (Japan)"
    );
}

#[test]
fn fallbacks_drop_trailing_tags_but_keep_region() {
    assert_eq!(fallback_names("X (USA) (Rev 1)"), vec!["X (USA)"]);
    assert_eq!(
        fallback_names("X (USA) (Rev 1) (Beta)"),
        vec!["X (USA) (Rev 1)", "X (USA)"]
    );
    assert_eq!(fallback_names("X (Japan) (En)"), vec!["X (Japan)"]);
    assert!(fallback_names("Chrono Trigger (USA)").is_empty());
    assert!(fallback_names("No Tags At All").is_empty());
}

#[test]
fn resolve_names_lists_exact_first_then_sanitised_fallbacks() {
    assert_eq!(
        resolve_names("X (USA) (Rev 1)"),
        vec!["X (USA) (Rev 1)", "X (USA)"]
    );
    assert_eq!(
        resolve_names("Q*bert's Qubes (USA) (Rev 1)"),
        vec!["Q_bert's Qubes (USA) (Rev 1)", "Q_bert's Qubes (USA)"]
    );
    assert_eq!(
        resolve_names("  Chrono Trigger (USA) "),
        vec!["Chrono Trigger (USA)"]
    );
}

#[test]
fn trailing_dot_and_inner_parentheses_survive() {
    // Title ending in a dot right before the region tag.
    assert_eq!(resolve_names("Mr. Do! (USA)"), vec!["Mr. Do! (USA)"]);
    assert_eq!(
        resolve_names("Super Bomberman 5 (Japan) (Rev 1)."),
        vec!["Super Bomberman 5 (Japan) (Rev 1)."]
    );
    // Parenthesised words inside the title are not tags.
    assert_eq!(
        fallback_names("Game (The) - Part 2 (Europe) (Rev 2)"),
        vec!["Game (The) - Part 2 (Europe)"]
    );
}
