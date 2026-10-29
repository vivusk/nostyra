//! Windows major.minor for the User-Agent via `RtlGetVersion` (ntdll): unlike
//! `GetVersionEx` it is not capped at 6.2 by the application manifest.

#[repr(C)]
struct OsVersionInfoW {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform_id: u32,
    service_pack: [u16; 128],
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(info: *mut OsVersionInfoW) -> i32;
}

/// `(major, minor)`, e.g. `(10, 0)` on Windows 10 and 11.
pub fn windows_version() -> Option<(u32, u32)> {
    let mut info = OsVersionInfoW {
        size: std::mem::size_of::<OsVersionInfoW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform_id: 0,
        service_pack: [0; 128],
    };
    // SAFETY: `info` is a valid, sized OSVERSIONINFOW out-param; returns NTSTATUS 0 on success.
    let status = unsafe { RtlGetVersion(&mut info) };
    (status == 0).then_some((info.major, info.minor))
}
