//! Chrome Native Messaging host. Reads length-prefixed JSON, sanitizes URL to hostname,
//! and atomically publishes only the active tab context.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Deserialize)]
struct Message {
    browser: String,
    session: String,
    window_id: i64,
    tab_id: i64,
    url: String,
}
#[derive(Serialize)]
struct Snapshot<'a> {
    browser: &'a str,
    session: &'a str,
    window_id: i64,
    tab_id: i64,
    domain: String,
    timestamp_ms: u64,
}

fn domain(url: &str) -> Option<String> {
    let authority = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?
        .split(&['/', '?', '#'][..])
        .next()?;
    let host = authority
        .split('@')
        .next_back()?
        .split(':')
        .next()?
        .to_ascii_lowercase();
    if host.is_empty()
        || host.len() > 253
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
    {
        return None;
    }
    Some(host)
}

fn handle(msg: Message) -> io::Result<()> {
    if !matches!(
        msg.browser.as_str(),
        "chrome.exe" | "msedge.exe" | "brave.exe"
    ) || msg.session.len() > 128
        || msg.tab_id < 0
    {
        return Ok(());
    }
    let Some(domain) = domain(&msg.url) else {
        return Ok(());
    };
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let snapshot = Snapshot {
        browser: &msg.browser,
        session: &msg.session,
        window_id: msg.window_id,
        tab_id: msg.tab_id,
        domain,
        timestamp_ms,
    };
    let dir = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
        .join("UVie");
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join("browser-context.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(&snapshot).map_err(io::Error::other)?,
    )?;
    std::fs::rename(&tmp, dir.join("browser-context.json"))
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
        let ok = serde_json::from_slice::<Message>(&data).map(handle).is_ok();
        let reply = serde_json::to_vec(&serde_json::json!({"ok": ok})).map_err(io::Error::other)?;
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
    fn strips_sensitive_url_parts() {
        assert_eq!(
            domain("https://Example.com:443/path?token=secret"),
            Some("example.com".into())
        );
        assert!(domain("chrome://settings").is_none());
    }
}
