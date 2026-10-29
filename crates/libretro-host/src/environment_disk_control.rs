//! Disk control (multi-disc games): the host announces interface version 1, so cores
//! register the EXT callback table; the legacy table is accepted too. The table is
//! copied (cores may hand a stack copy) and used by `CoreSession::discs`.

use std::ffi::{c_uint, c_void};

use crate::ffi_disc_message_types::*;
use crate::host_context::HostContext;

/// # Safety
/// `data` must be null or point to the type libretro.h documents for `cmd`.
pub(crate) unsafe fn disk_control(ctx: &mut HostContext, cmd: c_uint, data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    match cmd {
        RETRO_ENVIRONMENT_GET_DISK_CONTROL_INTERFACE_VERSION => {
            // SAFETY: `unsigned *` out-param.
            unsafe { data.cast::<c_uint>().write(1) };
            true
        }
        RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE => {
            // SAFETY: `const struct retro_disk_control_ext_callback *`.
            ctx.disk_control = Some(unsafe { *data.cast::<RetroDiskControlExtCallback>() });
            true
        }
        RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE => {
            // SAFETY: `const struct retro_disk_control_callback *`.
            let legacy = unsafe { *data.cast::<RetroDiskControlCallback>() };
            ctx.disk_control = Some(legacy.into());
            true
        }
        _ => false,
    }
}
