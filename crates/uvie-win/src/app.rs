//! Application wiring — hook → dispatcher → injector, plus tray menu,
//! focus tracking, settings persistence. Mirrors `UVieMacApp` +
//! `EventTap` orchestration in uvie-mac.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use uvie_core::dispatcher::{Context, Dispatch, Dispatcher, InputLanguage};
use uvie_core::engine_session::{EngineOptions, EngineSession};
use uvie_core::keys::KeyEvent;
use uvie_core::macros::MacroTable;
use uvie_core::memory::LanguageMemory;
use uvie_core::settings::{LanguagePref, Settings};
use windows::Win32::Foundation::HWND;

use crate::foreground::{self, FocusWatcher};
use crate::inject;
use crate::keyboard::{self, KeyboardHook};
use crate::pointer::PointerHook;
use crate::startup;
use crate::tray::{self, TrayIcon};

/// Settings/memory file locations (`%APPDATA%\UVie\`).
pub struct Paths {
    pub settings: PathBuf,
    pub memory: PathBuf,
    /// Macro-definition file (`macros.json`): written by the settings
    /// window, loaded once here at startup.
    pub macros: PathBuf,
}

impl Paths {
    pub fn appdata() -> Self {
        let dir = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("UVie");
        Self {
            settings: dir.join("settings.json"),
            memory: dir.join("memory.json"),
            macros: dir.join("macros.json"),
        }
    }
}

struct Shared {
    dispatcher: Dispatcher,
    settings: Settings,
    memory: LanguageMemory,
    foreground_exe: String,
    paths: Paths,
    /// Last-seen mtimes — the settings window writes the same files from
    /// another thread in this process; the tray timer polls these.
    settings_mtime: Option<SystemTime>,
    macros_mtime: Option<SystemTime>,
    /// Tray window handle, needed to re-register the global hotkey when
    /// `Settings.hotkey` changes.
    tray_hwnd: HWND,
}

impl Shared {
    fn apply_engine_options(&mut self) {
        let s = &self.settings;
        self.dispatcher.session.apply_options(EngineOptions {
            quick_start: s.quick_start,
            quick_telex: s.quick_telex,
            modern_orthography: s.modern_orthography,
            relaxed_coda: s.relaxed_coda,
            english_override: s.english_override,
        });
        self.dispatcher.session.set_input_method(s.input_method);
    }

    /// Persist app-owned state. The app only *reads* `settings.json` — it is
    /// written exclusively by the settings window — so saving it here would
    /// clobber UI edits made after startup.
    fn save(&self) {
        let _ = self.memory.save(&self.paths.memory);
    }

    /// Re-read settings/macros when their files changed on disk (the
    /// settings window writes them from another thread in this process;
    /// hand-edits are picked up too). Parse failures keep the old values —
    /// the writer may be mid-write.
    fn reload_changed(&mut self) {
        if let Some(new) = reload_json::<Settings>(&self.paths.settings, &mut self.settings_mtime) {
            let hotkey_changed = new.hotkey != self.settings.hotkey;
            let login_changed = new.launch_at_login != self.settings.launch_at_login;
            self.settings = new;
            self.apply_engine_options();
            self.dispatcher.macro_enabled = self.settings.macro_enabled;
            self.dispatcher.auto_capitalize = self.settings.auto_capitalize;
            if hotkey_changed && !self.tray_hwnd.is_invalid() {
                tray::update_hotkey(self.tray_hwnd, &self.settings.hotkey);
            }
            if login_changed {
                let _ = startup::set_launch_at_login(self.settings.launch_at_login);
            }
        }
        if let Some(macros) = reload_json::<MacroTable>(&self.paths.macros, &mut self.macros_mtime)
        {
            self.dispatcher.macros = macros;
        }
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Load a JSON file only when its mtime differs from `last`; `last` is
/// advanced only on a successful *parse*, so a torn read (the settings
/// window mid-write) retries on the next tick instead of latching the
/// mtime and going stale until the following save.
fn reload_json<T: DeserializeOwned>(path: &Path, last: &mut Option<SystemTime>) -> Option<T> {
    let mtime = file_mtime(path)?;
    if *last == Some(mtime) {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str(&text).ok()?;
    *last = Some(mtime);
    Some(value)
}

/// Owns the OS plumbing. Lives on the message-loop thread.
pub struct App {
    shared: Rc<RefCell<Shared>>,
    _hook: KeyboardHook,
    _pointer: Option<PointerHook>,
    _tray: TrayIcon,
    _focus: Option<FocusWatcher>,
}

impl App {
    pub fn new() -> windows::core::Result<Self> {
        let paths = Paths::appdata();
        let settings = Settings::load(&paths.settings);
        let memory = LanguageMemory::load(&paths.memory);

        let mut dispatcher = Dispatcher::new(EngineSession::new());
        dispatcher.macros = MacroTable::load(&paths.macros);
        dispatcher.macro_enabled = settings.macro_enabled;
        dispatcher.auto_capitalize = settings.auto_capitalize;
        let shared = Rc::new(RefCell::new(Shared {
            dispatcher,
            settings,
            memory,
            foreground_exe: String::new(),
            settings_mtime: file_mtime(&paths.settings),
            macros_mtime: file_mtime(&paths.macros),
            tray_hwnd: HWND::default(),
            paths,
        }));
        shared.borrow_mut().apply_engine_options();

        // -- keyboard hook -------------------------------------------------
        let shared_key = Rc::clone(&shared);
        let hook = KeyboardHook::install(move |key: KeyEvent| {
            let mut s = shared_key.borrow_mut();
            // Never process keys destined for our own windows (settings,
            // dialogs, tray menus) — otherwise Telex marks would rewrite
            // what the user types into UVie's own UI.
            let self_focused = foreground_belongs_to_self();
            let ctx = Context {
                language: None,
                app_excluded: self_focused
                    || s.settings.is_excluded(&s.foreground_exe)
                    || !s.settings.enabled,
                non_latin_layout: keyboard::active_layout_is_non_latin(),
            };
            let chromium = s.settings.is_chromium(&s.foreground_exe);
            match s.dispatcher.handle(key, &ctx) {
                Dispatch::Pass(plan) => {
                    inject::inject(&plan, chromium);
                    false
                }
                Dispatch::Consume(plan) => {
                    inject::inject(&plan, chromium);
                    true
                }
            }
        })?;

        // -- tray ----------------------------------------------------------
        let shared_menu = Rc::clone(&shared);
        let hotkey = shared.borrow().settings.hotkey.clone();
        let tray = TrayIcon::create(&hotkey, move |cmd| {
            let mut s = shared_menu.borrow_mut();
            match cmd {
                tray::CMD_TOGGLE_LANGUAGE => {
                    let lang = match s.dispatcher.language {
                        InputLanguage::Vietnamese => InputLanguage::English,
                        InputLanguage::English => InputLanguage::Vietnamese,
                    };
                    s.dispatcher.set_language(lang);
                    if s.settings.per_app_language && !s.foreground_exe.is_empty() {
                        let pref = match lang {
                            InputLanguage::Vietnamese => LanguagePref::Vietnamese,
                            InputLanguage::English => LanguagePref::English,
                        };
                        let exe = s.foreground_exe.clone();
                        s.memory.remember(&exe, pref);
                    }
                    s.save();
                }
                tray::CMD_OPEN_SETTINGS => {
                    crate::ui::open_settings();
                }
                tray::CMD_QUIT => unsafe {
                    windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
                },
                tray::CMD_SYNC_SETTINGS => s.reload_changed(),
                _ => {}
            }
        })?;
        shared.borrow_mut().tray_hwnd = tray.hwnd();

        // -- foreground-app watcher -----------------------------------------
        // Our own exe name, so the focus watcher can ignore the tray
        // window stealing foreground (it would otherwise attribute
        // language toggles to uvie-win.exe).
        let own_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "uvie-win.exe".to_string());

        let shared_focus = Rc::clone(&shared);
        let own_exe_watcher = own_exe.clone();
        // Non-fatal: a failed WinEvent hook only loses exclusion lists,
        // per-app language and the Chromium workaround — the IME itself
        // still works.
        let focus = FocusWatcher::install(move |exe| {
            apply_foreground(&mut shared_focus.borrow_mut(), exe, &own_exe_watcher);
        })
        .ok();

        // Seed the foreground app: the WinEvent hook only fires on focus
        // *changes*, so without this the app frontmost at launch stays
        // unclassified (no exclusion, no per-app language recall, no
        // Chromium workaround) until the first window switch.
        {
            let exe = foreground::foreground_exe();
            apply_foreground(&mut shared.borrow_mut(), exe, &own_exe);
        }

        // -- pointer hook ---------------------------------------------------
        // A click can move the caret where the single-line anchor model
        // cannot track — drop the composition like uvie-mac's event tap.
        // Non-fatal: losing it only means a click won't reset a stale
        // caret anchor.
        let shared_pointer = Rc::clone(&shared);
        let pointer = PointerHook::install(move || {
            shared_pointer.borrow_mut().dispatcher.on_pointer_down();
        })
        .ok();

        Ok(Self {
            shared,
            _hook: hook,
            _pointer: pointer,
            _tray: tray,
            _focus: focus,
        })
    }

    /// Persist the launch-at-login preference into the Run key.
    pub fn sync_launch_at_login(&self) {
        let s = self.shared.borrow();
        let _ = startup::set_launch_at_login(s.settings.launch_at_login);
    }
}

/// Apply a foreground-app change: remember the new exe, recall its Vi/En
/// preference, and drop the in-flight composition. Shared by the WinEvent
/// focus watcher and the startup seed. `own_exe` keeps our own windows
/// (settings, tray) from being attributed as the active app, and a
/// same-app hop must not churn the composition or sentence memory.
fn apply_foreground(s: &mut Shared, exe: String, own_exe: &str) {
    if exe.eq_ignore_ascii_case(own_exe) || exe.eq_ignore_ascii_case(&s.foreground_exe) {
        return;
    }
    s.foreground_exe = exe;
    // Focus change drops the in-flight composition and swaps the per-app
    // sentence-start state; the dispatcher contract calls for this
    // BEFORE restoring the per-app language.
    let app = s.foreground_exe.clone();
    s.dispatcher.on_app_switch(&app);
    if s.settings.per_app_language {
        let exe = s.foreground_exe.clone();
        if let Some(pref) = s.memory.recall(&exe) {
            let lang = match pref {
                LanguagePref::Vietnamese => InputLanguage::Vietnamese,
                LanguagePref::English => InputLanguage::English,
            };
            s.dispatcher.set_language(lang);
        }
    }
}

/// True when the foreground window belongs to this process — used to keep
/// the engine from rewriting keys typed into our own UI.
fn foreground_belongs_to_self() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
        pid != 0 && pid == std::process::id()
    }
}
