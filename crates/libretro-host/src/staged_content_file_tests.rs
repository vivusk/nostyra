//! Unit tests for `staged_content_file`.

use super::*;

fn temp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("nostyra-stage-")
        .tempdir()
        .unwrap()
}

#[test]
fn keeps_the_extension_and_deletes_on_drop() {
    let temp = temp();
    let dir = temp.path().join("1234");
    let staged = StagedContentFile::write(&dir, Some("gg"), b"patched").unwrap();
    assert_eq!(staged.path(), dir.join("content.gg"));
    assert_eq!(std::fs::read(staged.path()).unwrap(), b"patched");
    drop(staged);
    assert!(!dir.join("content.gg").exists());
    assert!(!dir.exists(), "empty staging dir removed too");
}

#[test]
fn no_extension_gives_a_bare_name() {
    let temp = temp();
    let staged = StagedContentFile::write(temp.path(), None, b"x").unwrap();
    assert_eq!(staged.path(), temp.path().join("content"));
    let staged = StagedContentFile::write(temp.path(), Some(""), b"x").unwrap();
    assert_eq!(staged.path(), temp.path().join("content"));
}

#[test]
fn unwritable_dir_is_a_stage_error() {
    let temp = temp();
    let file = temp.path().join("not-a-dir");
    std::fs::write(&file, b"").unwrap();
    let err = StagedContentFile::write(&file.join("sub"), Some("md"), b"x").unwrap_err();
    assert!(matches!(err, HostError::StageContent { .. }), "{err}");
}

#[test]
fn staged_path_of_256_bytes_is_refused_before_writing() {
    let temp = temp();
    let base = temp.path().to_string_lossy().len();
    // `<temp>/<pad>/content.md`: pad so the whole path is exactly 255, then 256 bytes.
    let pad = |total: usize| "y".repeat(total - base - "/content.md".len() - 1);
    let ok = temp.path().join(pad(255));
    let staged = StagedContentFile::write(&ok, Some("md"), b"x").unwrap();
    assert_eq!(staged.path().to_string_lossy().len(), 255);
    let long = temp.path().join(pad(256));
    let err = StagedContentFile::write(&long, Some("md"), b"x").unwrap_err();
    assert!(matches!(err, HostError::StageContent { .. }), "{err}");
    assert!(!long.exists(), "nothing written");
}
