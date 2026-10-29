//! Compiles the vendored rcheevos v12.5.0 (`third-party/rcheevos`) into a static lib,
//! plus the tiny C layout probe the FFI layout tests compare `#[repr(C)]` structs with.

use std::path::PathBuf;

/// rcheevos v12.5.0 tag layout (`src/Makefile.common` + `rc_libretro.c`).
const SOURCES: &[&str] = &[
    "rc_client.c",
    "rc_compat.c",
    "rc_libretro.c",
    "rc_util.c",
    "rc_version.c",
    "rapi/rc_api_common.c",
    "rapi/rc_api_editor.c",
    "rapi/rc_api_info.c",
    "rapi/rc_api_runtime.c",
    "rapi/rc_api_user.c",
    "rcheevos/alloc.c",
    "rcheevos/condition.c",
    "rcheevos/condset.c",
    "rcheevos/consoleinfo.c",
    "rcheevos/format.c",
    "rcheevos/lboard.c",
    "rcheevos/memref.c",
    "rcheevos/operand.c",
    "rcheevos/rc_validate.c",
    "rcheevos/richpresence.c",
    "rcheevos/runtime.c",
    "rcheevos/runtime_progress.c",
    "rcheevos/trigger.c",
    "rcheevos/value.c",
    "rhash/cdreader.c",
    "rhash/hash.c",
    "rhash/hash_disc.c",
    "rhash/hash_rom.c",
    "rhash/hash_zip.c",
    "rhash/md5.c",
];

/// Same defines for the library and the layout probe, so struct layouts agree.
fn configure(build: &mut cc::Build, third_party: &std::path::Path) {
    let rc = third_party.join("rcheevos");
    build
        .include(rc.join("include"))
        .include(rc.join("src"))
        .include(rc.join("src/rcheevos"))
        .include(third_party.join("libretro"))
        .define("RC_CLIENT_SUPPORTS_HASH", None)
        .define("RC_HASH_NO_ENCRYPTED", None)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        // MSVC /W4: 32 benign narrowing warnings (C4244/C4245) in upstream code.
        .flag_if_supported("/wd4244")
        .flag_if_supported("/wd4245");
}

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let third_party = manifest.join("../../third-party");
    let rc = third_party.join("rcheevos");
    println!("cargo:rerun-if-changed={}", rc.display());

    let mut build = cc::Build::new();
    for file in SOURCES {
        build.file(rc.join("src").join(file));
    }
    configure(&mut build, &third_party);
    build.warnings(true).compile("rcheevos");

    let probe = manifest.join("src/ffi/layout_probe.c");
    println!("cargo:rerun-if-changed={}", probe.display());
    let mut probe_build = cc::Build::new();
    probe_build.file(&probe);
    configure(&mut probe_build, &third_party);
    probe_build.compile("rcheevos_layout_probe");
}
