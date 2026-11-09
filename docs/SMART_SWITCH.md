# UVie Smart Switch (Windows)

Only `VI` and `EN` exist. Changing the language hotkey in a browser tab remembers that tab; browser-memory.json supports domain defaults. Per-app memory remains supported.

## Architecture

- `uvie-win`: Win32 foreground detection + keyboard hook; once per second polls `browser-context.json`, resolves `tab > domain > application`, and clears composition on tab changes.
- `uvie-native-host`: Chromium Native Messaging, length-prefixed stdio protocol. Stores only hostname, browser executable, tab/window IDs, random session ID, timestamp. Does not store URL paths, tokens, keystrokes, or page content.
- `browser-extension`: Chromium MV3 service worker reports active tab and sends periodic heartbeat.

## Browser companion setup

See [Chrome / Edge / Brave install instructions](../browser-extension/README.md). The `scripts/install-native-host.ps1` script registers the companion without administrator rights.

## Precedence

1. Last manual VI/EN toggle for the current browser tab, saved in `%APPDATA%\UVie\browser-memory.json`.
2. Domain preference set from the extension popup, in the same file.
3. Existing per-application preference.

## Window correlation

The Native Messaging host validates the real foreground browser process and matches the tab title against its top-level Windows window title before writing a snapshot with HWND. The UVie agent verifies that HWND is the current foreground window. This prevents other Chrome windows or Edge tabs from stealing another window's VI/EN preference. Unsupported window-title configurations safely fall back to app memory.

## Data minimization

The extension sends hostname, tab/window identifiers, window title and temporary session ID. No page content, full URL path/query, passwords or keystrokes are stored. No remote service is used.

## Test status

Rust cross-target compile and unit tests run on Ubuntu, but real Windows keyboard-hook and browser Native Messaging E2E still require validation before stable release.
