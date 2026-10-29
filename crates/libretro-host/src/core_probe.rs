//! `probe_core`: checks that a file is a loadable libretro core and reads its
//! identity, without `retro_init` or a game. The launcher runs this through
//! `nostyra-runner --probe-core` so a crashing core never takes the launcher down.

use std::path::Path;

use crate::core_library_loader::{CoreInfo, CoreLibrary};
use crate::host_error::HostResult;

/// Loads the library, resolves every `retro_*` symbol, checks the API version and
/// returns `retro_get_system_info` identity. The library is unloaded on return.
pub fn probe_core(path: &Path) -> HostResult<CoreInfo> {
    let library = CoreLibrary::open(path)?;
    let (_need_fullpath, info) = library.system_info();
    Ok(info)
}
