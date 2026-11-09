# UVie Smart Switch Companion (Chrome / Edge / Brave)

The extension sends **only the active tab hostname, tab/window identifiers and window title** to a local Native Messaging host. It does not access page contents, record keys or contact any external server.

## Windows installation

1. Build binaries on Windows from the repository root: `cargo build --release -p uvie-win -p uvie-native-host`.
2. Open `chrome://extensions` or `edge://extensions`, enable **Developer mode**, choose **Load unpacked**, and select `browser-extension/`.
3. Copy the extension's 32-character ID. Open PowerShell in the repository root and register the native host:

   ```powershell
   .\scripts\install-native-host.ps1 -Browser Chrome -ExtensionId abcdefghijklmnopabcdefghijklmnop
   # For Microsoft Edge (replace with Edge's own extension ID):
   .\scripts\install-native-host.ps1 -Browser Edge -ExtensionId abcdefghijklmnopabcdefghijklmnop
   ```

4. Reload the extension. Run `uvie-win.exe`. Open a regular HTTPS page and click the extension icon; it should report **Connected to UVie**.
5. Use the UVie VI/EN keyboard shortcut in a tab to remember that tab's choice, or use the extension popup to save a **domain-wide VI/EN default**.

Native Messaging is per-browser. Each browser needs its own extension ID and host registration. No admin permissions are required.

## Behavior and limitations

- Tab-specific manual choices have priority over domain rules, then app memory. Domain rule changes may take up to 1 second to propagate to UVie.
- The Native Host verifies the actual foreground browser executable and window title, then records the foreground HWND. UVie will only use context when its current foreground HWND matches. Incompatible browser title customizations may cause fallback to app-level preference.
- For safety, `chrome://`, `edge://`, internal pages, or tabs without an accessible title are not tracked. The fallback is the app's stored VI/EN mode.
- Browser tab sessions are scoped to a browser session (not guaranteed stable across browser restarts). Domain rules persist.
- The extension does not change UVie's active language by itself. Only the local app updates keyboard processing.
- `Native Host unavailable` means the host is not registered, not found, or rejected by the browser. Open **Service worker → Inspect** on the extensions page to diagnose errors.
- This is an experimental release. Do not use production-wide until Windows end-to-end validation is finished.
