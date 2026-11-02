//! Bridge to the settings window. The Slint view (`uvie-slintui`) is the
//! default; building with `--features winui` restores the WinUI 3 window
//! (`uvie-winui` crate). Either way the UI runs on its own dedicated
//! thread — see the respective crate docs.

#[cfg(feature = "winui")]
static UI: std::sync::OnceLock<()> = std::sync::OnceLock::new();

#[cfg(not(feature = "winui"))]
static UI: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>> = std::sync::Mutex::new(None);

/// Open (or focus) the settings window. Called from the tray menu,
/// double-click on the icon, and once at startup.
pub fn open_settings() {
    #[cfg(feature = "winui")]
    {
        let _ = UI.get_or_init(|| {
            let paths = crate::app::Paths::appdata();
            uvie_winui::spawn_settings_thread(paths.settings, paths.macros);
        });
    }
    #[cfg(not(feature = "winui"))]
    {
        let mut guard = UI.lock().unwrap();
        let alive = guard.as_ref().is_some_and(|tx| tx.send(()).is_ok());
        if !alive {
            // First open, or the UI thread died — (re)spawn it. The window
            // shows itself once built, so no extra wake-up is needed.
            let paths = crate::app::Paths::appdata();
            *guard = Some(uvie_slintui::spawn_settings_thread(
                paths.settings,
                paths.macros,
            ));
        }
    }
}
