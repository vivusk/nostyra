use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::*;

/// Writes a log `age_min` minutes old (write time decides "newest").
fn log_aged(dir: &Path, name: &str, text: &str, age_min: u64) {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    let modified = SystemTime::now() - Duration::from_secs(60 * age_min);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
}

fn zip_entries(path: &Path) -> Vec<(String, String)> {
    let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|index| {
            let mut file = archive.by_index(index).unwrap();
            let mut text = String::new();
            file.read_to_string(&mut text).unwrap();
            (file.name().unwrap().to_string(), text)
        })
        .collect()
}

#[test]
fn report_holds_system_info_two_launcher_and_five_runner_logs_scrubbed() {
    let dir = tempfile::tempdir().unwrap();
    for age in 0..3 {
        log_aged(
            dir.path(),
            &format!("launcher-2026102{age}.log"),
            "launcher line\n",
            age,
        );
    }
    for age in 0..7 {
        let text = format!("GET dorequest.php?u=Tester&t=tok123 session {age}\n");
        log_aged(
            dir.path(),
            &format!("runner-2026102{age}-nes.log"),
            &text,
            age,
        );
    }
    log_aged(dir.path(), "notes.txt", "private", 0);
    log_aged(dir.path(), "nostyra-bug-report-old.zip", "zip", 0);

    let path = write_bug_report(dir.path(), "Nostyra version: 0.1.0\n").unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with(BUG_REPORT_PREFIX) && name.ends_with(BUG_REPORT_EXT));

    let entries = zip_entries(&path);
    let names: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            SYSTEM_INFO_ENTRY,
            "launcher-20261020.log",
            "launcher-20261021.log",
            "runner-20261020-nes.log",
            "runner-20261021-nes.log",
            "runner-20261022-nes.log",
            "runner-20261023-nes.log",
            "runner-20261024-nes.log",
        ]
    );
    assert_eq!(entries[0].1, "Nostyra version: 0.1.0\n");
    for (name, text) in &entries[3..] {
        assert!(!text.contains("tok123"), "{name}: {text}");
        assert!(text.contains("t=<redacted>"), "{name}: {text}");
    }
}

#[test]
fn report_without_logs_still_has_system_info() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    let path = write_bug_report(&logs, "info\n").unwrap();
    let entries = zip_entries(&path);
    assert_eq!(
        entries,
        [(SYSTEM_INFO_ENTRY.to_string(), "info\n".to_string())]
    );
}
