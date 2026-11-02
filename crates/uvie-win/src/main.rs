// GUI subsystem — no console window when launched from Explorer or the
// Run key (parity with uvie-mac's `LSUIElement` agent).
#![windows_subsystem = "windows"]

//! uvie-win — Vietnamese input method agent for Windows.
//!
//! A tray-resident app like uvie-mac's `LSUIElement` agent: no main window,
//! a `WH_KEYBOARD_LL` hook feeds keystrokes through the `uvie` engine and
//! `SendInput` writes the Vietnamese text back.

mod app;
mod foreground;
mod inject;
mod keyboard;
mod startup;
mod tray;
mod ui;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

fn main() -> windows::core::Result<()> {
    let app = app::App::new()?;
    app.sync_launch_at_login();

    // Open the settings window on launch — same UX as OpenKey, and gives
    // immediate visual feedback when started from Explorer or login.
    ui::open_settings();

    // Standard Win32 message loop — the keyboard hook, tray window, and
    // WinEvent hook all deliver on this thread.
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
