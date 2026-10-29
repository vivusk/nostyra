//! M2 exit criterion "cover art for ≥ 90 % of No-Intro-matched games",
//! measured offline: every game name in the bundled SNES DAT → `resolve_names`
//! → the committed Named_Boxarts snapshot
//! (`scripts/update-thumbnail-index-fixture.{sh,ps1}` refreshes it). Asserted
//! on retail names (users own retail dumps); the all-names figure is printed.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use artwork_cache::{is_retail_name, resolve_names};

const BOXART_NAMES: &str = include_str!("fixtures/snes-boxart-names.txt");
const MIN_RETAIL_COVERAGE: f64 = 0.90;

fn dat_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../third-party/libretro-database/Nintendo - Super Nintendo Entertainment System.dat",
    )
}

/// One DAT game: the name thumbnails are looked up by, and the tags that
/// decide "retail" (game name plus first ROM file name: libretro's DAT keeps
/// `(Aftermarket)` / `(Unl)` only on the ROM name).
struct DatGame {
    name: String,
    retail: bool,
}

/// Unique games from `game ( name "…"` lines and their `rom ( name "…"` line.
fn dat_games() -> Vec<DatGame> {
    let dat = std::fs::read_to_string(dat_path()).expect("bundled SNES DAT");
    let quoted = |rest: &str| rest.split_once('"').map(|(value, _)| value.to_owned());
    let mut games: BTreeMap<String, bool> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut in_game_header = false; // skips the `clrmamepro ( name …` block
    for line in dat.lines().map(str::trim_start) {
        if line == "game (" {
            in_game_header = true;
        } else if let Some(name) = line
            .strip_prefix("name \"")
            .filter(|_| std::mem::take(&mut in_game_header))
            .and_then(quoted)
        {
            games.entry(name.clone()).or_insert(is_retail_name(&name));
            current = Some(name);
        } else if let Some(rom) = line.strip_prefix("rom ( name \"").and_then(quoted) {
            if let Some(retail) = current.take().and_then(|name| games.get_mut(&name)) {
                *retail &= is_retail_name(&rom);
            }
        }
    }
    games
        .into_iter()
        .map(|(name, retail)| DatGame { name, retail })
        .collect()
}

/// Share of `names` with at least one candidate in the snapshot.
fn coverage<'a>(
    games: impl Iterator<Item = &'a DatGame>,
    boxarts: &HashSet<&str>,
) -> (usize, usize) {
    games.fold((0, 0), |(hit, total), game| {
        let found = resolve_names(&game.name)
            .iter()
            .any(|c| boxarts.contains(c.as_str()));
        (hit + usize::from(found), total + 1)
    })
}

fn percent((hit, total): (usize, usize)) -> f64 {
    hit as f64 / total as f64
}

#[test]
fn retail_boxart_coverage_meets_exit_criterion() {
    let header = BOXART_NAMES.lines().next().unwrap();
    assert!(
        header.starts_with("# libretro-thumbnails"),
        "fixture header: {header}"
    );
    let boxarts: HashSet<&str> = BOXART_NAMES
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect();
    assert!(
        boxarts.len() > 3000,
        "snapshot looks truncated: {}",
        boxarts.len()
    );

    let games = dat_games();
    assert!(
        games.len() > 3000,
        "DAT parse found only {} games",
        games.len()
    );
    let retail = coverage(games.iter().filter(|game| game.retail), &boxarts);
    let all = coverage(games.iter(), &boxarts);
    for region in ["(USA)", "(Europe)", "(Japan)"] {
        let subset = games
            .iter()
            .filter(|game| game.retail && game.name.contains(region));
        let stats = coverage(subset, &boxarts);
        println!(
            "{region} retail: {}/{} = {:.1} %",
            stats.0,
            stats.1,
            percent(stats) * 100.0
        );
    }
    println!("{header}");
    println!(
        "retail: {}/{} = {:.1} %",
        retail.0,
        retail.1,
        percent(retail) * 100.0
    );
    println!(
        "all names: {}/{} = {:.1} %",
        all.0,
        all.1,
        percent(all) * 100.0
    );

    assert!(
        percent(retail) >= MIN_RETAIL_COVERAGE,
        "retail boxart coverage {:.1} % < 90 %",
        percent(retail) * 100.0
    );
}
