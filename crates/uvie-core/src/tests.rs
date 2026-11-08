//! Windows settings-schema tests. Dispatch, engine, macro and per-app memory
//! tests live in the shared `uvie-host` crate.

use crate::settings::Settings;
use uvie_host::InputMethod;

#[test]
fn settings_json_roundtrip() {
    let s = Settings {
        input_method: InputMethod::Vni,
        quick_telex: true,
        excluded_apps: vec!["notepad.exe".into()],
        macro_enabled: true,
        chromium_apps: vec!["mybrowser.exe".into()],
        ..Default::default()
    };
    let text = serde_json::to_string(&s).unwrap();
    let back: Settings = serde_json::from_str(&text).unwrap();
    assert_eq!(back, s);
    assert!(back.is_excluded("NOTEPAD.EXE"));
}

#[test]
fn settings_defaults_ship_chromium_list() {
    // Mirrors uvie-mac's `defaultChromiumBrowsers`: a fresh profile knows
    // the mainstream Chromium browsers and gets the overwrite workaround.
    let s = Settings::default();
    for exe in ["chrome.exe", "msedge.exe", "brave.exe", "arc.exe"] {
        assert!(s.is_chromium(exe), "{exe} should default to chromium list");
    }
    assert!(!s.is_chromium("notepad.exe"));
    assert!(!s.is_chromium("code.exe"));
    // Case-insensitive, like is_excluded.
    let s = Settings {
        chromium_apps: vec!["MyBrowser.EXE".into()],
        ..Default::default()
    };
    assert!(s.is_chromium("mybrowser.exe"));
}
