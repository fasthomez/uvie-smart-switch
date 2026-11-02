//! Windows App Runtime bootstrap for unpackaged processes.
//!
//! `MddBootstrapInitialize2` lives in `Microsoft.WindowsAppRuntime.Bootstrap.dll`,
//! a small flat-C loader shipped inside the WindowsAppSDK NuGet and copied
//! next to `uvie-win.exe` by the build script. We `LoadLibrary` it at
//! runtime and resolve the exports manually to avoid an import-library
//! dependency.

use std::path::PathBuf;
use windows::core::{Error, Result, HRESULT, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

#[allow(non_snake_case)]
type MddBootstrapInitialize2 = unsafe extern "system" fn(
    major_minor_version: u32,
    version_tag: PCWSTR,
    // PACKAGE_VERSION is a UINT64 union: Major<<48 | Minor<<32 | Build<<16 | Revision.
    min_version: u64,
    options: u32,
) -> HRESULT;

#[allow(non_snake_case)]
type MddBootstrapShutdown = unsafe extern "system" fn();

const WINDOWSAPPSDK_VERSION: u32 = (1 << 16) | 6; // 1.6
// WINDOWSAPPSDK_RUNTIME_VERSION_UINT64 from WindowsAppSDK-VersionInfo.h
// (WASDK 1.6.250108002 → runtime package 6000.373.1641.0).
const WINDOWSAPPSDK_MIN_VERSION: u64 = 0x1770_0175_0669_0000;
// MddBootstrapInitializeOptions::OnNoMatch_ShowUI: when no compatible
// runtime is installed, show the "install Windows App Runtime" dialog
// rather than silently failing (OnError_FailFast = 0x4 would kill the IME).
const ON_NO_MATCH_SHOW_UI: u32 = 0x0008;

/// Handle that keeps the bootstrap DLL loaded; `Drop` releases the runtime.
pub struct Bootstrap {
    dll: HMODULE,
    shutdown: MddBootstrapShutdown,
}

/// Look for `Microsoft.WindowsAppRuntime.Bootstrap.dll` next to the exe or
/// under the repo's fetched SDK dir (dev convenience via `UVIE_WASDK_DIR`).
fn find_bootstrap_dll() -> Option<PathBuf> {
    let name = "Microsoft.WindowsAppRuntime.Bootstrap.dll";
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    if let Ok(dir) = std::env::var("UVIE_WASDK_DIR") {
        let p = PathBuf::from(dir).join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn win32_err(code: u32, msg: &str) -> Error {
    Error::new(HRESULT::from_win32(code), msg)
}

impl Bootstrap {
    /// Initialize the Windows App Runtime for this unpackaged process.
    /// Errors when the bootstrapper isn't found — the UI should degrade
    /// gracefully rather than crash the IME.
    pub fn init() -> Result<Self> {
        let path = find_bootstrap_dll().ok_or_else(|| {
            win32_err(
                0x80070002,
                "Microsoft.WindowsAppRuntime.Bootstrap.dll not found (install the Windows App SDK runtime or run scripts/fetch-windowsappsdk.ps1)",
            )
        })?;

        unsafe {
            let wide: Vec<u16> = path
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let dll = LoadLibraryW(PCWSTR::from_raw(wide.as_ptr()))?;

            let init_addr = GetProcAddress(dll, windows::core::s!("MddBootstrapInitialize2"))
                .ok_or_else(|| {
                    let _ = FreeLibrary(dll);
                    win32_err(0x8007007F, "MddBootstrapInitialize2 not found")
                })?;
            let shutdown_addr = GetProcAddress(dll, windows::core::s!("MddBootstrapShutdown"))
                .ok_or_else(|| {
                    let _ = FreeLibrary(dll);
                    win32_err(0x8007007F, "MddBootstrapShutdown not found")
                })?;
            let init: MddBootstrapInitialize2 = std::mem::transmute(init_addr);
            let shutdown: MddBootstrapShutdown = std::mem::transmute(shutdown_addr);

            let hr = init(
                WINDOWSAPPSDK_VERSION,
                PCWSTR::null(),
                WINDOWSAPPSDK_MIN_VERSION,
                ON_NO_MATCH_SHOW_UI,
            );
            if let Err(e) = hr.ok() {
                let _ = FreeLibrary(dll);
                return Err(e);
            }
            Ok(Self { dll, shutdown })
        }
    }
}

impl Drop for Bootstrap {
    fn drop(&mut self) {
        unsafe {
            (self.shutdown)();
            let _ = FreeLibrary(self.dll);
        }
    }
}
