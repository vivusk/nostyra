//! A [`FakeTransport`] configured from a folder (the runner's debug
//! `NOSTYRA_RA_FAKE_SERVER=<dir>` mode and the canned sets in `test-assets/achievements`):
//! `<kind>.json` = answer body, `<kind>.status` = its HTTP status (default 200; `-2` = no
//! response, e.g. an offline award), `game-hash.txt` = the only hash the game set is
//! answered for.

use std::path::Path;

use crate::fake_server_transport::{FakeReply, FakeTransport};

/// Optional file: the RA hash the canned `achievementsets` answer belongs to.
pub const FAKE_GAME_HASH_FILE: &str = "game-hash.txt";

impl FakeTransport {
    /// Defaults overridden by the folder's `<kind>.json` / `<kind>.status` files; with
    /// [`FAKE_GAME_HASH_FILE`] the set is answered for that hash only.
    pub fn from_dir(dir: &Path) -> std::io::Result<Self> {
        let fake = Self::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let (Some(kind), Some(ext)) = (
                path.file_stem().and_then(|stem| stem.to_str()),
                path.extension().and_then(|ext| ext.to_str()),
            ) else {
                continue;
            };
            if ext != "json" && ext != "status" {
                continue;
            }
            let read = |ext: &str| std::fs::read_to_string(dir.join(format!("{kind}.{ext}")));
            let body = read("json").unwrap_or_default();
            let status = match read("status") {
                Ok(text) => text.trim().parse().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "bad status file")
                })?,
                Err(_) => 200,
            };
            fake.set_reply(kind, FakeReply::Respond(status, body));
        }
        if let Ok(hash) = std::fs::read_to_string(dir.join(FAKE_GAME_HASH_FILE)) {
            fake.answer_set_only_for(hash.trim());
        }
        Ok(fake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server_transport::{ServerRequest, ServerTransport};

    fn call(fake: &FakeTransport, post_data: &str) -> (i32, String) {
        let request = ServerRequest {
            url: "https://retroachievements.org/dorequest.php".into(),
            post_data: post_data.into(),
            content_type: String::new(),
        };
        let response = fake.call(&request);
        (response.status, response.body)
    }

    #[test]
    fn folder_sets_bodies_statuses_and_the_known_hash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("achievementsets.json"), "{\"set\":1}").unwrap();
        std::fs::write(dir.path().join("awardachievement.status"), "-2\n").unwrap();
        std::fs::write(dir.path().join("game-hash.txt"), "ABCD\n").unwrap();
        let fake = FakeTransport::from_dir(dir.path()).unwrap();
        assert_eq!(
            call(&fake, "r=achievementsets&m=abcd"),
            (200, "{\"set\":1}".into())
        );
        let other = call(&fake, "r=achievementsets&m=ffff");
        assert_eq!(
            other.1,
            crate::FAKE_UNKNOWN_GAME_JSON,
            "other hash: unknown game"
        );
        assert_eq!(call(&fake, "r=awardachievement&a=1"), (-2, String::new()));
        assert_eq!(call(&fake, "r=ping").0, 200, "defaults kept");
    }
}
