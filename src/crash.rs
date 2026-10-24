//! Crash reporting. Release builds have no console window, so a panic would
//! otherwise just close the game silently. This writes `crash.log` next to the
//! .exe and shows an error box with the reason.

use std::fmt::Write as _;
use std::path::PathBuf;

pub fn install() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let reason = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown error".into());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();

        let mut report = String::new();
        let _ = writeln!(report, "Honty Monty crashed.\n\nReason: {reason}\nWhere: {location}");
        if reason.to_lowercase().contains("gpu") || reason.to_lowercase().contains("adapter") {
            let _ = writeln!(
                report,
                "\nThis usually means the graphics driver doesn't support Vulkan or DirectX 12.\n\
                 Try updating your graphics driver."
            );
        }
        let log = log_path();
        let _ = std::fs::write(&log, &report);
        let _ = writeln!(report, "\nThis was saved to {}", log.display());

        show_error(&report);
        default_hook(info);
    }));
}

fn log_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("crash.log")))
        .unwrap_or_else(|| PathBuf::from("crash.log"))
}

#[cfg(windows)]
fn show_error(text: &str) {
    use std::ffi::c_void;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, caption) = (wide(text), wide("Honty Monty"));
    const MB_ICONERROR: u32 = 0x10;
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONERROR);
    }
}

#[cfg(not(windows))]
fn show_error(text: &str) {
    eprintln!("{text}");
}
