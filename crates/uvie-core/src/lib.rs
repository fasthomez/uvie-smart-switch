//! uvie-core — Windows facade over the shared `uvie-host` crate plus the
//! Windows-specific settings schema.
//!
//! Dispatch, engine sessions, macros and per-app memory live in `uvie-host`
//! (same repo/tag as the `uvie` engine). This crate re-exports them under the
//! historical module paths the Win32 front end and the Slint settings window
//! already use, and owns `settings` (Win32 virtual-key hotkeys, the
//! chromium exe list, UI mode).

pub use uvie_host::{dispatcher, engine_session, keys, macros, memory};
pub mod settings;

pub use dispatcher::{Context, Dispatch, Dispatcher, InputAction, InputLanguage};
pub use engine_session::{EngineOptions, EngineSession};
pub use keys::{KeyEvent, KeyKind};
pub use macros::{MacroEntry, MacroTable};
pub use memory::LanguageMemory;
pub use settings::{Hotkey, LanguagePref, Settings};
pub use uvie_host::InputMethod;

#[cfg(test)]
mod tests;
