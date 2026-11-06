//! How the web edition's registered content files are looked up. Virtual paths
//! (`/web/<folder>/<relative path>`) are built by joining what sheets say (cue `FILE`,
//! m3u lines) to their folder, so they compare like Windows paths on the desktop:
//! `\` is a separator, `.` / `..` are resolved, case is ignored.

use std::path::Path;

/// The lookup key of `path`: `/`-separated, `.` and `..` resolved, lower case.
pub fn lookup_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/")).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_case_dots_and_backslashes() {
        let key = lookup_key(Path::new("/web/ab12/PS1/Game.cue"));
        assert_eq!(key, "/web/ab12/ps1/game.cue");
        assert_eq!(lookup_key(Path::new("/web/ab12/PS1/./x/../GAME.CUE")), key);
        assert_eq!(lookup_key(Path::new("/web/ab12/PS1\\Game.cue")), key);
    }
}
