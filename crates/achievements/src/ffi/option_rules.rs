//! rcheevos' table of core options hardcore forbids (`rc_libretro.c`), keyed by the
//! core's `retro_system_info.library_name`.

use super::c_text::c_string;
use super::libretro_ffi::{rc_libretro_get_disallowed_settings, rc_libretro_is_setting_allowed};

/// Whether rcheevos allows `key = value` for the core named `library_name` (cores
/// without a table allow everything).
pub fn rc_setting_allowed(library_name: &str, key: &str, value: &str) -> bool {
    let (library, key, value) = (c_string(library_name), c_string(key), c_string(value));
    // SAFETY: NUL-terminated strings valid for the calls; the returned table is a
    // static array inside rcheevos (NULL when the core has none).
    unsafe {
        let list = rc_libretro_get_disallowed_settings(library.as_ptr());
        list.is_null() || rc_libretro_is_setting_allowed(list, key.as_ptr(), value.as_ptr()) != 0
    }
}
