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
mod pointer;
mod startup;
mod tray;
mod ui;

use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

fn main() -> windows::core::Result<()> {
    // Single instance per session: a second copy would double-process
    // every keystroke (two LL hooks feeding one engine, double-injected
    // output). A failed mutex means nothing is known — run unguarded
    // rather than refuse to start.
    let _instance = match unsafe { CreateMutexW(None, true, w!("Local\\UVieWin.SingleInstance")) } {
        Ok(_) if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS => return Ok(()),
        Ok(h) => h,
        Err(_) => HANDLE::default(),
    };

    let app = app::App::new()?;
    app.sync_launch_at_login();

    // Open the settings window on launch — same UX as OpenKey, and gives
    // immediate visual feedback when started from Explorer or login.
    ui::open_settings();

    // Standard Win32 message loop — the keyboard hook, tray window, and
    // WinEvent hook all deliver on this thread. GetMessageW returns -1 on
    // error; BOOL::as_bool() would treat that as true and spin, so test
    // the raw value.
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
