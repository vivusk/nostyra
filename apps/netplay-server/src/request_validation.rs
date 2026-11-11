//! Size / charset checks on client-supplied text (nicknames, the lobby game hint).

use crate::api_error::ApiError;
use crate::room_model::GameHint;

const MAX_NICKNAME_CHARS: usize = 32;
const MAX_TITLE_CHARS: usize = 200;
const MAX_ID_CHARS: usize = 64;

/// Trimmed nickname, 1..=32 chars, no control characters.
pub fn clean_nickname(raw: &str) -> Result<String, ApiError> {
    let nick = raw.trim();
    let count = nick.chars().count();
    if count == 0 || count > MAX_NICKNAME_CHARS || nick.chars().any(char::is_control) {
        return Err(ApiError::BadRequest("bad_nickname"));
    }
    Ok(nick.to_owned())
}

/// Bounded fields; `content_sha1` must be 40 hex chars (lowercased).
pub fn clean_game_hint(game: GameHint) -> Result<GameHint, ApiError> {
    let bad = || ApiError::BadRequest("bad_game");
    let sha1 = game.content_sha1.trim().to_ascii_lowercase();
    if sha1.len() != 40 || !sha1.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let text_ok =
        |s: &str, max: usize| s.chars().count() <= max && !s.chars().any(char::is_control);
    let ids_ok = [&game.system, &game.core_id, &game.core_version]
        .iter()
        .all(|s| text_ok(s, MAX_ID_CHARS));
    if !ids_ok || !text_ok(&game.title, MAX_TITLE_CHARS) {
        return Err(bad());
    }
    Ok(GameHint {
        content_sha1: sha1,
        ..game
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hint(sha1: &str, title: &str) -> GameHint {
        GameHint {
            content_sha1: sha1.into(),
            title: title.into(),
            system: "snes".into(),
            core_id: "snes9x".into(),
            core_version: "1.62".into(),
        }
    }

    #[test]
    fn nicknames() {
        assert_eq!(clean_nickname("  Vivi ").unwrap(), "Vivi");
        assert!(clean_nickname("   ").is_err());
        assert!(clean_nickname(&"x".repeat(33)).is_err());
        assert!(clean_nickname("a\nb").is_err());
        assert_eq!(clean_nickname("Ngọc Ánh").unwrap(), "Ngọc Ánh");
    }

    #[test]
    fn game_hints() {
        let ok = clean_game_hint(hint(&"AB".repeat(20), "Game")).unwrap();
        assert_eq!(ok.content_sha1, "ab".repeat(20));
        assert!(clean_game_hint(hint("abc", "Game")).is_err());
        assert!(clean_game_hint(hint(&"zz".repeat(20), "Game")).is_err());
        assert!(clean_game_hint(hint(&"ab".repeat(20), &"t".repeat(201))).is_err());
    }
}
