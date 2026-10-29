//! `tauri::generate_context!()` requires `frontendDist` (`../dist`) to exist at compile
//! time. Create it when missing so `cargo clippy/test --workspace` works on a fresh clone
//! without npm; the real UI comes from `npm run build`.

use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist");
    if let Err(err) = std::fs::create_dir_all(&dist) {
        panic!("cannot create {}: {err}", dist.display());
    }
    tauri_build::build();
}
