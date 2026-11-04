# AGENTS.md — notes for coding agents working on uvie-win

## Build quick reference

```powershell
cargo build                            # uvie-core + uvie-slintui + uvie-win
cargo run -p uvie-slintui --example preview  # open the Slint settings window standalone
cargo test -p uvie-core                # headless tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

## uvie-slintui notes

- UI is split: `ui/widgets.slint` (shared controls) + `ui/pages/*.slint`
  (one per pane) + `ui/settings.slint` (root window). Fluent style only —
  `build.rs` sets `with_style("fluent")` (material looked wrong; Slint
  styles are compile-time-only, runtime style switching would need a
  per-style compile+module dispatch — not worth it). Light/dark is live
  via `Palette.color-scheme` (`scheme-index` prop → `Settings::mode`).
- In slint 1.18: `Palette.accent` is now `Palette.accent-background`;
  top-level `function`s are illegal (use a `global`); assignments inside
  callbacks need braces (`clicked => { x = 1; }`); children of a plain
  `Rectangle` do NOT stretch — set `width/height: 100%`; `enabled` can't be
  redeclared on a component inheriting ScrollView (built-in prop).
- `match settings.borrow().theme` extended the RefCell borrow through the
  whole match → `borrow_mut` panic in UI callbacks. Bind it to a `let`
  first.
- `run_event_loop_until_quit()` keeps the loop alive after the window
  hides (close → `CloseRequestResponse::HideWindow`), so the tray can
  re-show it via a channel + `upgrade_in_event_loop`.
- Pane switching is instant on purpose — no opacity/slide animations.

## Hard-won constraints — read before touching versions

- **windows 0.62 wraps nullable handles in `Option<>`**: `SetWindowsHookExW`,
  `CallNextHookEx`, `CreateWindowExW`, `ToUnicodeEx`, `GetModuleFileNameExW`,
  `Reg*ExW` reserved args, `TrackPopupMenu` — pass `Some(..)`/`None`.
- **`WINEVENT_OUTOFCONTEXT` lives in `Win32::UI::WindowsAndMessaging`**, not
  `UI::Accessibility`.
- `EnableWindow` lives in `Win32::UI::Input::KeyboardAndMouse`, not
  `WindowsAndMessaging`; `GetStockObject` returns `HGDIOBJ` directly (not
  `Result`).
- The keyboard hook must skip keys when the foreground window belongs to
  this process (`GetForegroundWindow` + pid compare), or the engine rewrites
  what the user types into UVie's own settings UI/dialogs.
- **Settings/macros hot-reload is timer-based.** `tray.rs` runs a 1s
  `WM_TIMER` → `Shared::reload_changed` re-reads `settings.json` /
  `macros.json` when mtimes change (settings window writes them in-process).
  Touching either file requires no IPC — but anything that must react to a
  settings change (e.g. `RegisterHotKey`) has to be re-run in `reload_changed`.

## IME architecture notes

- The LL keyboard hook cannot see keys destined for **elevated** processes —
  those keystrokes pass through uncomposed (same as uvie-mac's secure-input
  limitation).
- Injected events are marked `LLKHF_INJECTED` and `dwExtraInfo =
  0x55564945` ("uVIE") so the hook swallows its own echoes.
- All Win32-facing code is in `uvie-win`; keep `uvie-core` platform-free so
  it stays unit-testable (see `crates/uvie-core/src/tests.rs`, which replays
  real Telex sequences validated against uvie-rs's own test vectors).
- `uvie-rs-version` records the engine pin; the `uvie` dependency tag in
  `Cargo.toml` must match it.

## CI

`.github/workflows/ci.yml` on `windows-latest`: fmt check, clippy `-D
warnings`, `cargo test`, `cargo build --release`, uploads `uvie-win.exe` +
Inno Setup installer as artifacts.
