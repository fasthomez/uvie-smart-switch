//! Browser context snapshots written by the native messaging companion.
//! Never collect page text or keystrokes; only hostname and tab identity.
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BrowserContext {
    pub browser: String,
    pub session: String,
    pub window_id: i64,
    pub tab_id: i64,
    pub domain: String,
    pub hwnd: isize,
    pub timestamp_ms: u64,
}

impl BrowserContext {
    pub fn load(path: &Path) -> Option<Self> {
        let value: Self = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        if value.hwnd == 0
            || value.tab_id < 0
            || value.session.len() > 128
            || value.domain.len() > 253
        {
            return None;
        }
        if !value
            .domain
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        {
            return None;
        }
        Some(value)
    }
    pub fn belongs_to(&self, foreground_exe: &str, foreground_hwnd: isize, now_ms: u64) -> bool {
        matches!(
            self.browser.as_str(),
            "chrome.exe" | "msedge.exe" | "brave.exe"
        ) && self.browser == foreground_exe
            && self.hwnd == foreground_hwnd
            && self.timestamp_ms <= now_ms
            && now_ms - self.timestamp_ms < 90_000
    }
    pub fn tab_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.browser, self.session, self.window_id, self.tab_id
        )
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct BrowserMemory {
    pub tabs: HashMap<String, bool>,
    pub domains: HashMap<String, bool>,
}

impl BrowserMemory {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|x| serde_json::from_slice(&x).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes)?;
        std::fs::rename(temp, path)
    }
    pub fn recall(&self, ctx: &BrowserContext) -> Option<bool> {
        self.tabs
            .get(&ctx.tab_key())
            .copied()
            .or_else(|| self.domains.get(&ctx.domain).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tab_overrides_domain() {
        let ctx = BrowserContext {
            browser: "chrome.exe".into(),
            session: "s".into(),
            window_id: 1,
            tab_id: 2,
            domain: "example.com".into(),
            hwnd: 123,
            timestamp_ms: 10,
        };
        let mut memory = BrowserMemory::default();
        memory.domains.insert("example.com".into(), true);
        memory.tabs.insert(ctx.tab_key(), false);
        assert_eq!(memory.recall(&ctx), Some(false));
        assert!(ctx.belongs_to("chrome.exe", 123, 100));
        assert!(!ctx.belongs_to("msedge.exe", 123, 100));
        assert!(!ctx.belongs_to("chrome.exe", 123, 90_011));
    }
}
