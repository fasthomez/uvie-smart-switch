//! Browser native messaging host: only hostname and verified foreground HWND cross the bridge.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Deserialize)]
struct Message {
    id: u64,
    command: String,
    browser: String,
    session: String,
    window_id: i64,
    tab_id: i64,
    domain: String,
    title: String,
    vi: Option<bool>,
}
#[derive(Serialize)]
struct Snapshot<'a> {
    browser: &'a str,
    session: &'a str,
    window_id: i64,
    tab_id: i64,
    hwnd: isize,
    domain: &'a str,
    timestamp_ms: u64,
}
#[derive(Serialize)]
struct Response {
    id: u64,
    ok: bool,
    mode: Option<bool>,
    error: Option<String>,
}

fn valid_domain(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.is_ascii()
        && host.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}

#[cfg(windows)]
fn active_window(title: &str) -> Option<(isize, String)> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path = [0u16; MAX_PATH as usize];
        let mut len = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        if result.is_err() {
            return None;
        }
        let exe = String::from_utf16_lossy(&path[..len as usize]);
        let exe = exe.rsplit(['\\', '/']).next()?.to_ascii_lowercase();
        if !matches!(exe.as_str(), "chrome.exe" | "msedge.exe" | "brave.exe") {
            return None;
        }
        // A same-browser background window cannot be attributed by process name alone.
        // Match the tab title to the active top-level browser window title.
        if title.is_empty() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let n = GetWindowTextW(hwnd, &mut buf);
        if n == 0 {
            return None;
        }
        let window_title = String::from_utf16_lossy(&buf[..n as usize]);
        if !window_title.starts_with(title) {
            return None;
        }
        Some((hwnd.0 as isize, exe))
    }
}
#[cfg(not(windows))]
fn active_window(_title: &str) -> Option<(isize, String)> {
    None
}

fn data_dir() -> std::path::PathBuf {
    std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
        .join("UVie")
}
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(temp, path)
}
fn handle(msg: Message) -> Response {
    let mut reply = Response {
        id: msg.id,
        ok: false,
        mode: None,
        error: None,
    };
    if !matches!(
        msg.browser.as_str(),
        "chrome.exe" | "msedge.exe" | "brave.exe"
    ) || msg.session.len() > 128
        || msg.tab_id < 0
        || !valid_domain(&msg.domain)
        || msg.title.len() > 2048
    {
        reply.error = Some("Invalid browser context".into());
        return reply;
    }
    let dir = data_dir();
    let result: io::Result<()> = (|| {
        std::fs::create_dir_all(&dir)?;
        // Even preference updates require a verified focused browser window.
        // This also prevents service workers from changing another window's state.
        let (hwnd, actual_browser) = active_window(&msg.title)
            .ok_or_else(|| io::Error::other("No matching foreground browser window"))?;
        let memory_path = dir.join("browser-memory.json");
        match msg.command.as_str() {
            "active" | "status" => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let snap = Snapshot {
                    browser: &actual_browser,
                    session: &msg.session,
                    window_id: msg.window_id,
                    tab_id: msg.tab_id,
                    hwnd,
                    domain: &msg.domain,
                    timestamp_ms: now,
                };
                atomic_write(
                    &dir.join("browser-context.json"),
                    &serde_json::to_vec(&snap).map_err(io::Error::other)?,
                )?;
            }
            "set_domain" | "clear_domain" => {
                let mut memory: serde_json::Value = std::fs::read(&memory_path)
                    .ok()
                    .and_then(|x| serde_json::from_slice(&x).ok())
                    .unwrap_or_else(|| serde_json::json!({"tabs":{},"domains":{}}));
                let domains = memory
                    .as_object_mut()
                    .and_then(|m| m.get_mut("domains"))
                    .and_then(|v| v.as_object_mut())
                    .ok_or_else(|| io::Error::other("Invalid memory schema"))?;
                if msg.command == "set_domain" {
                    domains.insert(
                        msg.domain.clone(),
                        serde_json::json!(msg
                            .vi
                            .ok_or_else(|| io::Error::other("Missing VI/EN mode"))?),
                    );
                } else {
                    domains.remove(&msg.domain);
                }
                atomic_write(
                    &memory_path,
                    &serde_json::to_vec_pretty(&memory).map_err(io::Error::other)?,
                )?;
            }
            _ => return Err(io::Error::other("Unknown command")),
        }
        let memory: serde_json::Value = std::fs::read(memory_path)
            .ok()
            .and_then(|x| serde_json::from_slice(&x).ok())
            .unwrap_or_default();
        reply.mode = memory
            .get("domains")
            .and_then(|x| x.get(&msg.domain))
            .and_then(|v| v.as_bool());
        Ok(())
    })();
    match result {
        Ok(()) => reply.ok = true,
        Err(e) => reply.error = Some(e.to_string()),
    }
    reply
}
fn main() -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut len = [0u8; 4];
        if input.read_exact(&mut len).is_err() {
            break;
        }
        let size = u32::from_le_bytes(len) as usize;
        if size > 64 * 1024 {
            break;
        }
        let mut data = vec![0; size];
        input.read_exact(&mut data)?;
        let response = match serde_json::from_slice::<Message>(&data) {
            Ok(msg) => handle(msg),
            Err(_) => Response {
                id: 0,
                ok: false,
                mode: None,
                error: Some("Invalid JSON".into()),
            },
        };
        let reply = serde_json::to_vec(&response).map_err(io::Error::other)?;
        output.write_all(&(reply.len() as u32).to_le_bytes())?;
        output.write_all(&reply)?;
        output.flush()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostname_validation() {
        assert!(valid_domain("chatgpt.com"));
        assert!(valid_domain("proxmox.local"));
        assert!(!valid_domain("invalid.com:80/path?secret"));
        assert!(!valid_domain(""));
        assert!(!valid_domain("evil..com"));
    }
}
