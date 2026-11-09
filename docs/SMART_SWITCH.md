# UVie Smart Switch (Windows)

Only `VI` and `EN` exist. Changing the language hotkey in a browser tab remembers that tab; browser-memory.json supports domain defaults. Per-app memory remains supported.

## Architecture

- `uvie-win`: Win32 foreground detection + keyboard hook; once per second polls `browser-context.json`, resolves `tab > domain > application`, and clears composition on tab changes.
- `uvie-native-host`: Chromium Native Messaging, length-prefixed stdio protocol. Stores only hostname, browser executable, tab/window IDs, random session ID, timestamp. Does not store URL paths, tokens, keystrokes, or page content.
- `browser-extension`: Chromium MV3 service worker reports active tab and sends periodic heartbeat.

## Setup (development)

1. Build on Windows: `cargo build --release -p uvie-win -p uvie-native-host`.
2. Replace the example `path` in `browser-extension/native-host.example.json` with the full path to `uvie-native-host.exe`.
3. Load unpacked `browser-extension` in `chrome://extensions`, copy its extension ID.
4. Replace EXTENSION_ID in the Native Messaging manifest's `allowed_origins`.
5. Register the manifest path under `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.fasthomez.uvie_smart_switch` (default string value). Edge uses `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.fasthomez.uvie_smart_switch`.
6. Launch `uvie-win.exe`. Open two tabs, toggle the VI/EN hotkey separately, and switch between them.

Set a domain default in `%APPDATA%\UVie\browser-memory.json`, for example `{ "domains": { "remote.example.com": false }, "tabs": {} }`. `true` means VI; `false` means EN. Restart UVie after editing external memory.

## Limitations / validation needed

- Prototype currently associates active tab with browser **executable**, not precise Windows HWND. Multiple windows of the same browser may produce incorrect attribution; a future native window correlation layer is needed before production release.
- Extension does not know exact browser identity for Brave if its UA masks Brave as Chrome; configure browser-specific variant or improve native host binding.
- Browser startup/reconnection relies on a 30s heartbeat; stale context expires after 90s and falls back to app memory.
- Actual Windows binary and browser end-to-end tests remain required.
- Reuses upstream `uvie-win` and `uvie-rs`, preserving their MIT/Apache-2.0 licenses and notices.
