//! A save folder shared with the web edition: only the desktop's own `auto.*` is a
//! resume point, and its SRAM check reads `save.srm`, never `save.web.srm`.

use super::*;

fn web_sidecar() -> StateSidecar {
    StateSidecar {
        core_name: "snes9x".into(),
        core_version: "emulatorjs-4.2.3".into(),
        created_at: 1_700_000_000,
        sram_crc32: Some(crc32fast::hash(b"sram")),
        ..StateSidecar::default()
    }
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("nostyra-resume-web-")
        .tempdir()
        .unwrap()
}

#[test]
fn a_web_resume_point_is_not_a_desktop_one() {
    let temp = scratch();
    let dir = temp.path();
    std::fs::write(dir.join("web-auto.state"), [0u8; 16]).unwrap();
    let json = serde_json::to_vec(&web_sidecar()).unwrap();
    std::fs::write(dir.join("web-auto.json"), &json).unwrap();
    std::fs::write(dir.join("auto.state.crswap"), b"half").unwrap();
    assert_eq!(
        read_resume_point(dir, None, &StateLoadRule::default()),
        None
    );
}

#[test]
fn sram_warning_ignores_the_web_only_sram() {
    let temp = scratch();
    let dir = temp.path();
    let sidecar = StateSidecar {
        core_name: "Snes9x".into(),
        core_version: "1.63".into(),
        ..web_sidecar()
    };
    std::fs::write(dir.join("auto.state"), [0u8; 16]).unwrap();
    std::fs::write(dir.join("auto.json"), serde_json::to_vec(&sidecar).unwrap()).unwrap();
    std::fs::write(dir.join(SRAM_FILE_NAME), b"sram").unwrap();
    std::fs::write(
        dir.join(ipc_protocol::WEB_SRAM_FILE_NAME),
        b"newer web sram",
    )
    .unwrap();
    let point = read_resume_point(dir, None, &StateLoadRule::default()).unwrap();
    assert!(point.warnings.is_empty(), "{:?}", point.warnings);
}
