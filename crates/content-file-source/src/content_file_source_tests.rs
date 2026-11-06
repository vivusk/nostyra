#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use super::*;

#[test]
fn os_files_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    std::fs::write(&path, b"hello").unwrap();
    assert!(is_file(&path));
    assert!(!is_file(dir.path()), "a folder is not a file");
    assert_eq!(len(&path).unwrap(), 5);
    assert_eq!(read_to_string(&path).unwrap(), "hello");
    let missing = open(&dir.path().join("none")).err().unwrap();
    assert_eq!(missing.kind(), io::ErrorKind::NotFound);
}

#[test]
fn installed_source_serves_virtual_paths_then_is_removed() {
    let mut files = MemoryFiles::new();
    files.insert("/web/1/game.cue", b"FILE \"game.bin\" BINARY".to_vec());
    files.insert("/web/1/game.bin", vec![7u8; 10]);
    let cue = Path::new("/web/1/game.cue");
    let seen = with_source(Rc::new(files), || {
        let mut file = open(Path::new("/web/1/game.bin")).unwrap();
        file.seek(SeekFrom::Start(8)).unwrap();
        let mut rest = Vec::new();
        file.read_to_end(&mut rest).unwrap();
        assert_eq!(stream_len(file.as_mut()).unwrap(), 10);
        (read_to_string(cue).unwrap(), rest, is_file(cue))
    });
    assert_eq!(seen.0, "FILE \"game.bin\" BINARY");
    assert_eq!(seen.1, vec![7, 7]);
    assert!(seen.2);
    assert!(!is_file(cue), "source restored after the call");
}

#[test]
fn source_restored_after_a_panic() {
    let result = std::panic::catch_unwind(|| {
        with_source(Rc::new(MemoryFiles::new()), || panic!("boom"));
    });
    assert!(result.is_err());
    assert!(SOURCE.with(|cell| cell.borrow().is_none()));
}
