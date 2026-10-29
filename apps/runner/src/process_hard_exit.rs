//! Ends the process without running exit handlers, after a graphics failure (exit 6).
//! A core whose GL context could not be created is kept loaded (its unload crashes,
//! M12 phase-00 spike); PPSSPP's static destructors then abort a normal exit with
//! `0xC0000409`, so the launcher would see a crash instead of exit 6. Output is
//! flushed first; nothing else is left to save (the game never started). Also the
//! way out of a hanging core teardown (`teardown_watchdog`) and the normal end of
//! every session of a `keep_library_loaded` core (Dolphin), after its unload, save
//! writes and reporting: its DLL-detach phase can hang a normal process exit.

use std::io::Write;

pub mod teardown_watchdog;

/// Flushes stdout/stderr, then terminates with `code` (Windows `TerminateProcess`,
/// Unix `_exit`).
#[allow(unsafe_code)] // TerminateProcess / _exit: no safe std equivalent
pub fn exit_now(code: u8) -> ! {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut c_void;
            fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
        }
        // SAFETY: the pseudo-handle of the current process; terminating it does not
        // return on success.
        unsafe { TerminateProcess(GetCurrentProcess(), u32::from(code)) };
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn _exit(status: i32) -> !;
        }
        // SAFETY: POSIX `_exit`: ends the process without atexit handlers.
        unsafe { _exit(i32::from(code)) }
    }
    #[allow(unreachable_code)] // only if TerminateProcess failed
    std::process::exit(i32::from(code))
}
