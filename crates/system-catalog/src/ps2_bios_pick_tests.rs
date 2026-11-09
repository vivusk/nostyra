//! Picker: region match, any-region dumps, newest fallback, name tie-break, none.

use super::*;
use crate::ps2_bios_romdir::Ps2BiosVersion;

fn dump(name: &str, major: u8, minor: u8, date: &str, region: char) -> BiosFileStatus {
    let version = Ps2BiosVersion {
        major,
        minor,
        date: date.to_owned(),
        region,
        console: true,
    };
    BiosFileStatus {
        file_name: name.to_owned(),
        known: None,
        region: version.region_code(),
        valid: true,
        label: Some(version.label()),
        ps2_version: Some(version),
    }
}

fn report(files: Vec<BiosFileStatus>) -> FirmwareReport {
    FirmwareReport { files }
}

fn name(choice: Option<&BiosFileStatus>) -> Option<&str> {
    choice.map(|f| f.file_name.as_str())
}

#[test]
fn region_match_beats_a_newer_dump_of_another_region() {
    let r = report(vec![
        dump("scph39001.bin", 1, 60, "2002-02-07", 'A'),
        dump("scph50004.bin", 1, 90, "2003-04-22", 'E'),
        dump("scph10000.bin", 1, 0, "2000-01-17", 'J'),
    ]);
    assert_eq!(name(choose(&r, Some("US"))), Some("scph39001.bin"));
    assert_eq!(name(choose(&r, Some("EU"))), Some("scph50004.bin"));
    assert_eq!(name(choose(&r, Some("JP"))), Some("scph10000.bin"));
}

#[test]
fn newest_wins_within_a_region_and_as_the_fallback() {
    let r = report(vec![
        dump("scph39001.bin", 1, 60, "2002-02-07", 'A'),
        dump("scph70012.bin", 2, 0, "2004-06-14", 'A'),
        dump("scph77001.bin", 2, 20, "2006-02-10", 'A'),
        dump("scph90001.bin", 2, 30, "2008-02-20", 'A'),
    ]);
    assert_eq!(name(choose(&r, Some("US"))), Some("scph90001.bin"));
    assert_eq!(
        name(choose(&r, Some("JP"))),
        Some("scph90001.bin"),
        "no JP: newest"
    );
    assert_eq!(
        name(choose(&r, None)),
        Some("scph90001.bin"),
        "unknown region"
    );
}

#[test]
fn any_region_dumps_count_for_every_region_and_ties_go_by_name() {
    let r = report(vec![
        dump("b-devel.bin", 1, 60, "2002-02-07", 'T'),
        dump("a-devel.bin", 1, 60, "2002-02-07", 'T'),
    ]);
    assert_eq!(
        name(choose(&r, Some("EU"))),
        Some("b-devel.bin"),
        "report order"
    );
    let mut sorted = r.clone();
    sorted.files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    assert_eq!(name(choose(&sorted, Some("EU"))), Some("a-devel.bin"));
}

#[test]
fn invalid_or_empty_reports_choose_nothing() {
    assert_eq!(choose(&report(vec![]), Some("US")), None);
    let mut broken = dump("x.bin", 1, 60, "2002-02-07", 'A');
    broken.valid = false;
    assert_eq!(choose(&report(vec![broken]), Some("US")), None);
}

#[test]
fn disc_region_from_video_mode_then_serial_prefix() {
    assert_eq!(disc_region(Some(Ps2VideoMode::Pal), None), Some("EU"));
    assert_eq!(
        disc_region(Some(Ps2VideoMode::Pal), Some("SLUS-20312")),
        Some("EU"),
        "PAL wins over the serial"
    );
    assert_eq!(
        disc_region(Some(Ps2VideoMode::Ntsc), Some("SLUS-20312")),
        Some("US")
    );
    assert_eq!(
        disc_region(Some(Ps2VideoMode::Ntsc), Some("SCUS-97399")),
        Some("US")
    );
    assert_eq!(
        disc_region(Some(Ps2VideoMode::Ntsc), Some("SLPS-25497")),
        Some("JP")
    );
    assert_eq!(disc_region(None, Some("SLPM-65002")), Some("JP"));
    assert_eq!(disc_region(None, Some("SCAJ-20001")), Some("JP"));
    assert_eq!(disc_region(None, Some("SLKA-25001")), Some("JP"));
    assert_eq!(disc_region(None, Some("sles-54150")), Some("EU"));
    assert_eq!(
        disc_region(Some(Ps2VideoMode::Ntsc), Some("NSTR-00001")),
        None
    );
    assert_eq!(disc_region(None, Some("SL")), None, "short serial");
    assert_eq!(disc_region(None, None), None);
}
