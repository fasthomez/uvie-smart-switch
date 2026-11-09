//! WH_MOUSE_LL low-level mouse hook — the Windows counterpart of the
//! pointer-down half of uvie-mac's CGEventTap.
//!
//! A button-down can move the caret somewhere the dispatcher's single-line
//! anchor model cannot track, so every press drops the in-flight
//! composition (`Dispatcher::on_pointer_down`). Synthetic clicks are NOT
//! filtered: an automation-injected click still moves the real caret.

use std::cell::RefCell;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, WH_MOUSE_LL, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_XBUTTONDOWN,
};

type ClickHandler = RefCell<Option<Box<dyn FnMut()>>>;

thread_local! {
    /// Runs on the message-loop thread, same as the keyboard hook.
    static CLICK_HANDLER: ClickHandler = const { RefCell::new(None) };
}

pub struct PointerHook(HHOOK);

impl PointerHook {
    /// `on_down` runs on every mouse button press (left, right, middle, X).
    pub fn install(on_down: impl FnMut() + 'static) -> windows::core::Result<Self> {
        CLICK_HANDLER.with(|h| {
            *h.borrow_mut() = Some(Box::new(on_down));
        });
        let hhook = unsafe {
            // Same deal as the keyboard hook: a null module handle is valid
            // for LL hooks whose proc lives in this exe.
            SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0)?
        };
        Ok(Self(hhook))
    }
}

impl Drop for PointerHook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
        CLICK_HANDLER.with(|h| *h.borrow_mut() = None);
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        if matches!(
            msg,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        ) {
            let _ = CLICK_HANDLER.try_with(|h| {
                if let Ok(mut g) = h.try_borrow_mut() {
                    if let Some(f) = g.as_mut() {
                        f();
                    }
                }
            });
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
