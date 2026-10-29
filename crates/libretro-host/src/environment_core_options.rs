//! `GET_VARIABLE`: answers the core options the host fixes for this session
//! (registry `CoreSpec.fixed_options` + runner `--core-option`); any other key gets
//! `false`, so the core keeps its own default. Values never change in a session,
//! so `GET_VARIABLE_UPDATE` always answers "no update".

use std::ffi::{c_void, CStr};

use crate::ffi_disc_message_types::RetroVariable;
use crate::host_context::HostContext;

/// # Safety
/// `data` must be null or point to a `struct retro_variable` whose `key` is null or a
/// NUL-terminated string.
pub(crate) unsafe fn get_variable(ctx: &HostContext, data: *mut c_void) -> bool {
    let var = data.cast::<RetroVariable>();
    if var.is_null() {
        return false;
    }
    // SAFETY: non-null `struct retro_variable *` per the libretro contract.
    let key = unsafe { (*var).key };
    if key.is_null() {
        return false;
    }
    // SAFETY: the core passes a NUL-terminated key.
    let key = unsafe { CStr::from_ptr(key) };
    let Some((_, value)) = ctx.core_options.iter().find(|(k, _)| k.as_c_str() == key) else {
        return false;
    };
    // SAFETY: out-field of the same struct; the CString lives as long as the session,
    // which the libretro contract requires of the returned pointer.
    unsafe { (*var).value = value.as_ptr() };
    true
}
