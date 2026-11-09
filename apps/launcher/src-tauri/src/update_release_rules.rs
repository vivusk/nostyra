//! Pure rules of the update checker: which GitHub release counts as newer, whether it
//! carries the Windows portable zip, the plain-text notes excerpt shown in the banner,
//! and the URL allowlist for "Open release page". No I/O here, so every rule is tested.

use semver::Version;

/// GitHub "latest release" endpoint: excludes drafts, pre-releases and tag-only versions.
pub const RELEASE_FEED_URL: &str = "https://api.github.com/repos/vivusk/nostyra/releases/latest";
/// Fallback page when the cached release URL is missing or not allowlisted.
pub const RELEASES_LATEST_URL: &str = "https://github.com/vivusk/nostyra/releases/latest";
/// Only release pages of our own repository may be opened in the browser.
const RELEASE_PAGE_PREFIX: &str = "https://github.com/vivusk/nostyra/releases/";
/// Automatic checks run at most once a day (last success + 24 h).
pub const CHECK_INTERVAL_SECS: i64 = 86_400;
/// Banner excerpt cap in characters (the "…" comes on top).
const EXCERPT_MAX_CHARS: usize = 400;

/// `v0.4.0` / `V0.4.0` / `0.4.0` → version; anything that is not full semver → `None`.
pub fn parse_release_version(tag: &str) -> Option<Version> {
    let tag = tag.trim();
    let bare = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag);
    Version::parse(bare).ok()
}

/// Semver precedence (`0.5.0-rc.1 < 0.5.0`); build metadata is ignored.
pub fn is_newer(latest: &Version, current: &Version) -> bool {
    latest.cmp_precedence(current).is_gt()
}

/// Asset name `release.ps1` uploads for `version`.
pub fn portable_zip_name(version: &Version) -> String {
    format!("Nostyra-{version}-windows-x64-portable.zip")
}

/// True when the release already has its portable zip (it is attached after the
/// release is created, so a fresh release may briefly have none).
pub fn has_portable_zip<'a>(
    asset_names: impl IntoIterator<Item = &'a str>,
    version: &Version,
) -> bool {
    let wanted = portable_zip_name(version);
    asset_names.into_iter().any(|name| name == wanted)
}

/// `url` when it is one of our release pages, else the generic latest-release page, so
/// an odd or tampered feed can never open an arbitrary site.
pub fn safe_release_url(url: &str) -> &str {
    if url.starts_with(RELEASE_PAGE_PREFIX) {
        url
    } else {
        RELEASES_LATEST_URL
    }
}

/// Plain-text excerpt of the release notes: the first prose paragraph under the first
/// `## What's new` heading, else the first prose paragraph of the body. Markdown
/// emphasis, code ticks, links and list markers are stripped; at most 400 characters,
/// cut at a word, plus "…". Empty body → "".
pub fn notes_excerpt(body: &str) -> String {
    let blocks = markdown_blocks(body);
    let after_heading = blocks
        .iter()
        .position(|block| is_whats_new_heading(block[0]))
        .and_then(|at| blocks[at + 1..].iter().find(|block| is_prose(block)));
    let Some(block) = after_heading.or_else(|| blocks.iter().find(|block| is_prose(block))) else {
        return String::new();
    };
    let text: Vec<String> = block.iter().map(|line| strip_inline(line)).collect();
    truncate_at_word(&collapse_whitespace(&text.join(" ")))
}

/// Groups trimmed non-empty lines into paragraphs; each heading is its own block.
fn markdown_blocks(body: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in body.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
            if !line.is_empty() {
                blocks.push(vec![line]);
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn is_whats_new_heading(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.starts_with("## what's new") || lower.starts_with("## what\u{2019}s new")
}

/// Not a heading, quote or table (the release body starts with a download table).
fn is_prose(block: &[&str]) -> bool {
    block
        .first()
        .is_some_and(|line| !line.starts_with(['#', '>', '|']))
}

/// One line without list marker, `**` / `__` / backticks, `[text](url)` → `text`.
fn strip_inline(line: &str) -> String {
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .unwrap_or(line);
    let line = line.replace("**", "").replace("__", "").replace('`', "");
    strip_links(&line)
}

/// `[text](url)` → `text`; brackets without a following `(url)` stay as they are.
fn strip_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let link = after.find(']').and_then(|close| {
            let url_part = after[close + 1..].strip_prefix('(')?;
            url_part
                .find(')')
                .map(|end| (&after[..close], &url_part[end + 1..]))
        });
        match link {
            Some((text, tail)) => {
                out.push_str(text);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// ≤ 400 characters, cut at the last space inside the cap, plus "…".
fn truncate_at_word(text: &str) -> String {
    if text.chars().count() <= EXCERPT_MAX_CHARS {
        return text.to_owned();
    }
    let cap: String = text.chars().take(EXCERPT_MAX_CHARS).collect();
    let cut = cap.rfind(' ').map_or(cap.as_str(), |at| &cap[..at]);
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
#[path = "update_release_rules_tests.rs"]
mod tests;
