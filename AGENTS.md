# AGENTS.md — notes for coding agents working on uvie-win

## Build quick reference

```powershell
pwsh scripts/fetch-windowsappsdk.ps1   # once, or when bumping the SDK pin / building winui
cargo build                            # uvie-core + uvie-slintui + uvie-win (uvie-winui excluded, see below)
cargo build -p uvie-win --features winui  # build with the legacy WinUI settings window
cargo run -p uvie-slintui --example preview  # open the Slint settings window standalone
cargo test -p uvie-core                # headless tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

The Slint settings window (`uvie-slintui`) is the default UI. `uvie-winui`
remains a workspace member for the `--features winui` fallback, but is
excluded from `default-members` because its `windows-bindgen` step takes
minutes (~1.4M-line `bindings.rs`). `--workspace` still covers it.

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

## Hard-won constraints — read before touching versions

- **windows-bindgen must match the windows-core family.** Bindgen `0.62.1`
  pairs with `windows`/`windows-core` `0.62.x`. Bindgen 0.100 generates code
  for windows-core 0.100 internals (`imp::Type`, `EventRevoker`,
  `ArrayProxy`) that do not exist in 0.62 → ~900k errors. Keep the trio in
  `Cargo.toml` `[workspace.dependencies]` aligned.
- **`--no-deps` is required.** The published `windows-collections`/`windows-future`/
  `windows-numerics` crates do NOT contain every generated type (e.g.
  `IObservableVector`); the bindgen `--reference` stage deps produce
  unresolved imports. `--no-deps` generates everything self-contained in
  `OUT_DIR/bindings.rs`.
- **`--implement` is a bare flag** in bindgen 0.62 (no per-interface args).
- **`--no-allow` is required** because `include!`-ing a file that starts with
  `#![allow]` inside a `mod bindings` block is illegal; the allow attributes
  live on the `mod` declaration in `crates/uvie-winui/src/lib.rs`.
- **windows-core 0.62 array API**: `windows_core::imp::array_proxy(ptr, len)`
  returns `ArrayProxy<T>`; there is no `ArrayProxy::from_raw_parts`. The
  `build.rs` post-processor rewrites generated `.as_array()` call sites with
  balanced-paren matching — keep it if bindgen output changes shape.
- **windows 0.62 wraps nullable handles in `Option<>`**: `SetWindowsHookExW`,
  `CallNextHookEx`, `CreateWindowExW`, `ToUnicodeEx`, `GetModuleFileNameExW`,
  `Reg*ExW` reserved args, `TrackPopupMenu` — pass `Some(..)`/`None`.
- **`WINEVENT_OUTOFCONTEXT` lives in `Win32::UI::WindowsAndMessaging`**, not
  `UI::Accessibility`.
- **XAML callbacks need `Send + 'static`** — share state as
  `Arc<Mutex<Settings>>`, never `Rc<RefCell<..>>`.
- **`IInspectable` params** (Header, Content, ResourceDictionary keys, nav
  item Tags) take `PropertyValue::CreateString(&HSTRING)` boxing;
  `Ref::cast()` requires `use windows_core::Interface`.
- **Namespace filter additions that matter**: `Windows.Foundation` gives
  `TypedEventHandler` (NavigationView.SelectionChanged) and `Uri`
  (HyperlinkButton); `Windows.UI.Text` gives `FontWeight`. **`Windows.Graphics`
  and `Microsoft.Graphics` produce almost nothing** — `SizeInt32` isn't in the
  WASDK winmds, so `AppWindow.Resize`/`MoveAndResize` come out uncallable.
  Size/set the icon on the window via Win32 instead: `FindWindowW(title)` →
  `SetWindowPos` + `SendMessageW(WM_SETICON)` (see `dress_window()`).
- **`Grid::SetColumn` takes `Param<FrameworkElement>`**, not `UIElement` —
  cast children to `FrameworkElement` first.
- **COM identity compare**: for "which nav item got selected", cast both to
  `windows_core::IUnknown` and compare `Interface::as_raw` — comparing
  `IInspectable`/`NavigationViewItem` pointers directly can alias tear-offs.
- **XAML event handlers** (`SelectionChanged`, `Toggled`, `Click`, ...) need
  `FnMut(Ref<'_, S>, Ref<'_, A>) -> Result<()> + Send + 'static`; use
  `.ok()`/`.as_ref()` on the `Ref` args, and `args.SelectedItem()?` for the
  payload.
- **NavigationView and TextBox fast-fail on Server 2022 + WASDK 1.6.** Any
  control that spins up the XAML text-input path (TextBox, and
  NavigationView's pane search box) aborts the whole process at render time
  — no exception, no WER record, just a silent exit inside
  `Application::Start`'s loop. Bisected control-by-control: Grid/StackPanel/
  Border/ScrollViewer/TextBlock/Button/ToggleSwitch/ComboBox/FontIcon/
  HyperlinkButton all render fine. The settings UI therefore uses a
  hand-rolled sidebar (Border + Buttons) instead of NavigationView, and
  collects text through `entry_dialog::prompt` — a plain Win32 modal
  (STATIC/EDIT/BUTTON) that never touches XAML.
- `EnableWindow` lives in `Win32::UI::Input::KeyboardAndMouse`, not
  `WindowsAndMessaging`; `BS_DEFPUSHBUTTON` is an `i32` const (or into the
  style mask, don't wrap in `WINDOW_STYLE`); `GetStockObject` returns
  `HGDIOBJ` directly (not `Result`).
- The tray's hidden helper window shares the title "UVie for Windows" —
  `FindWindowW` by title alone can return the wrong one. Match the XAML
  window by class: `WinUIDesktopWin32WindowClass` + title.
- The keyboard hook must skip keys when the foreground window belongs to
  this process (`GetForegroundWindow` + pid compare), or the engine rewrites
  what the user types into UVie's own settings UI/dialogs.
- **`bash` on Windows PATH is often WSL's bash.** The fetch script must work
  in both Git Bash and WSL (unzip → bsdtar → python3/python fall-through;
  GNU tar cannot read zip).
- **Settings/macros hot-reload is timer-based.** `tray.rs` runs a 1s
  `WM_TIMER` → `Shared::reload_changed` re-reads `settings.json` /
  `macros.json` when mtimes change (settings window writes them in-process).
  Touching either file requires no IPC — but anything that must react to a
  settings change (e.g. `RegisterHotKey`) has to be re-run in `reload_changed`.
- **Known issue (winui feature only)**: closing the WinUI settings window
  kills the XAML thread and `Application::Start` can't run twice — the
  window can't reopen (Slint path hides instead of destroying). Fix would
  need `AppWindow.Closing` → cancel + `Hide()`.

## IME architecture notes

- The LL keyboard hook cannot see keys destined for **elevated** processes —
  those keystrokes pass through uncomposed (same as uvie-mac's secure-input
  limitation).
- Injected events are marked `LLKHF_INJECTED` and `dwExtraInfo =
  0x55564945` ("uVIE") so the hook swallows its own echoes.
- All Win32-facing code is in `uvie-win`; keep `uvie-core` platform-free so
  it stays unit-testable (see `crates/uvie-core/src/tests.rs`, which replays
  real Telex sequences validated against uvie-rs's own test vectors).
- Deps are fetched at dev time into `Frameworks/` and gitignored — mirror of
  uvie-mac's `scripts/fetch-uvie.sh` convention. `uvie-rs-version` records
  the engine pin; `Cargo.toml` must match it.

## CI

`.github/workflows/ci.yml` on `windows-latest`: fmt check, clippy `-D
warnings`, `cargo test --workspace`, `cargo build --release`, uploads
`uvie-win.exe` + bootstrap DLL as an artifact.
