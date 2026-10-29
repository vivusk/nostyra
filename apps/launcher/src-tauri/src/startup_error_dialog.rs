//! A launcher that cannot start says why. Release builds have no console, so without
//! this a missing WebView2 runtime (or an unwritable data folder) closes silently.

/// WebView2 Evergreen Runtime download page (Microsoft's permanent link).
const WEBVIEW2_DOWNLOAD_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

/// Shows the start failure in a native message box (Windows); logs it elsewhere.
pub fn show(err: &anyhow::Error) {
    let text = format!(
        "Nostyra could not start.\n\n{err:#}\n\nIf this PC lacks the Microsoft Edge WebView2 Runtime, \
         install it from:\n{WEBVIEW2_DOWNLOAD_URL}"
    );
    #[cfg(windows)]
    message_box("Nostyra", &text);
    #[cfg(not(windows))]
    eprintln!("{text}");
}

#[cfg(windows)]
#[allow(unsafe_code)] // Win32 MessageBoxW (startup failure before the webview exists)
fn message_box(title: &str, text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (title, text) = (wide(title), wide(text));
    // SAFETY: both buffers are NUL-terminated UTF-16 and outlive the call; no owner
    // window (null), so the box is modal to nothing.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
