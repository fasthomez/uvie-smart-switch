//! Foreground-app tracking — the Windows counterpart of uvie-mac's
//! `AppContextDetector` + `NSWorkspace.activated` observation.
//!
//! `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` notifies us whenever focus
//! changes; we resolve the owning process's exe name for the per-app
//! exclusion list and language memory.

use std::cell::RefCell;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT,
};

type FocusHandler = RefCell<Option<Box<dyn FnMut(String)>>>;

thread_local! {
    static FOCUS_HANDLER: FocusHandler = const { RefCell::new(None) };
}

pub struct FocusWatcher(HWINEVENTHOOK);

impl FocusWatcher {
    /// `on_focus` receives the lowercase exe name of the foreground process.
    pub fn install(on_focus: impl FnMut(String) + 'static) -> windows::core::Result<Self> {
        FOCUS_HANDLER.with(|h| *h.borrow_mut() = Some(Box::new(on_focus)));
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            return Err(windows::core::Error::from_thread());
        }
        Ok(Self(hook))
    }
}

impl Drop for FocusWatcher {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWinEvent(self.0);
        }
        FOCUS_HANDLER.with(|h| *h.borrow_mut() = None);
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.is_invalid() {
        return;
    }
    // Empty string when the owning process can't be resolved (elevated /
    // protected windows) — still delivered so a previously attributed exe
    // doesn't stick and wrongly apply that app's exclusion/language prefs.
    let exe = process_exe_name(hwnd).unwrap_or_default();
    FOCUS_HANDLER.with(|h| {
        if let Ok(mut g) = h.try_borrow_mut() {
            if let Some(f) = g.as_mut() {
                f(exe);
            }
        }
    });
}

/// Lowercase exe name of the process owning the current foreground
/// window, or "" when unresolvable. `EVENT_SYSTEM_FOREGROUND` only fires
/// on focus *changes*, so the app that was frontmost when UVie launched
/// would never be classified without this startup seed.
pub fn foreground_exe() -> String {
    unsafe { process_exe_name(GetForegroundWindow()).unwrap_or_default() }
}

/// exe file name (lowercase, e.g. `notepad.exe`) of the process that owns
/// `hwnd`, or None when it can't be resolved (elevated/system apps).
pub fn process_exe_name(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let mut len = buf.len() as u32;
        // QueryFullProcessImageNameW resolves the image path through the
        // kernel — needs only PROCESS_QUERY_LIMITED_INFORMATION and works
        // cross-bitness (GetModuleFileNameExW fails on 32-bit targets
        // from this 64-bit process, which would leave such apps
        // unclassified: no exclusion, per-app language or Chromium
        // workaround).
        let ok = QueryFullProcessImageNameW(
            proc,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(proc);
        if ok.is_err() || len == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/'])
            .next()
            .map(|s| s.to_ascii_lowercase())
    }
}
