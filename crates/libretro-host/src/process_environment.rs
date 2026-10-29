//! Process environment variables a core reads with `getenv` (`CoreSpec.process_env`,
//! e.g. PPSSPP `CI`, melonDS DS `MELONDSDS_SKIP_ERROR_SCREEN`).
//!
//! `std::env::set_var` updates the process environment block, which cores built
//! against the same CRT (MSVC, static) or a POSIX libc see. A MinGW core on Windows
//! reads `msvcrt.dll`'s own copy instead, taken when msvcrt started (already before
//! the runner's `main`), so the value is also put there with msvcrt `_putenv_s`
//! (M13 phase-00 spike: `set_var` alone is not seen by melonDS DS).

/// Sets `key=value` for the core about to load. Must run before the core library is
/// loaded and while no other thread reads the environment.
pub fn set_process_env(key: &str, value: &str) {
    std::env::set_var(key, value);
    #[cfg(windows)]
    if !msvcrt::put_env(key, value) {
        tracing::debug!(key, "msvcrt not loaded; process environment only");
    }
}

#[cfg(windows)]
mod msvcrt {
    use std::ffi::{c_char, c_int, CString};

    use libloading::os::windows::Library;

    type PutEnvS = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;

    /// `_putenv_s(key, value)` in an already-loaded `msvcrt.dll`; false when msvcrt is
    /// not loaded (it then copies the process environment when it does load) or the
    /// call fails.
    pub(super) fn put_env(key: &str, value: &str) -> bool {
        let (Ok(key), Ok(value)) = (CString::new(key), CString::new(value)) else {
            return false;
        };
        let Ok(lib) = Library::open_already_loaded("msvcrt.dll") else {
            return false;
        };
        // SAFETY: `_putenv_s` has this C signature in every msvcrt.dll.
        let Ok(put) = (unsafe { lib.get::<PutEnvS>(b"_putenv_s\0") }) else {
            return false;
        };
        // SAFETY: two NUL-terminated strings valid for the call; msvcrt copies them.
        unsafe { put(key.as_ptr(), value.as_ptr()) == 0 }
    }

    #[cfg(test)]
    mod tests {
        use std::ffi::{c_char, CStr, CString};

        use libloading::os::windows::Library;

        type GetEnv = unsafe extern "C" fn(*const c_char) -> *const c_char;

        /// msvcrt's own `getenv`, as a MinGW core calls it.
        fn msvcrt_getenv(lib: &Library, key: &str) -> Option<String> {
            let key = CString::new(key).unwrap();
            // SAFETY: C signature of `getenv`; the result is read before any change.
            unsafe {
                let get = lib.get::<GetEnv>(b"getenv\0").unwrap();
                let value = get(key.as_ptr());
                (!value.is_null()).then(|| CStr::from_ptr(value).to_string_lossy().into_owned())
            }
        }

        #[test]
        fn the_value_reaches_msvcrts_own_environment() {
            // Loaded (or kept loaded) for the test, as it is in the runner.
            // SAFETY: system DLL without unusual initialisation.
            let lib = unsafe { Library::new("msvcrt.dll") }.expect("msvcrt.dll");
            let key = "NOSTYRA_TEST_PROCESS_ENV";
            super::super::set_process_env(key, "1");
            assert_eq!(std::env::var(key).as_deref(), Ok("1"));
            assert_eq!(msvcrt_getenv(&lib, key).as_deref(), Some("1"));
            super::super::set_process_env(key, "2");
            assert_eq!(msvcrt_getenv(&lib, key).as_deref(), Some("2"));
        }
    }
}
