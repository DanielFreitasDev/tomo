//! App settings (`settings.toml` in the app config dir). Machine-owned file —
//! plain serde round-trip, no comment preservation needed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// UI locale (`"en"`, `"pt-BR"`); `None` = follow system.
    pub locale: Option<String>,
    pub ui_font_size: Option<u8>,
    pub editor_font_size: Option<u8>,
    pub network: NetworkSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSettings {
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub ssl_verify: bool,
    /// In-memory response cap; larger bodies spill to a temp file.
    pub response_cap_bytes: u64,
    pub proxy: ProxySettings,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            follow_redirects: true,
            max_redirects: 10,
            ssl_verify: true,
            response_cap_bytes: 10 * 1024 * 1024,
            proxy: ProxySettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProxySettings {
    pub mode: ProxyMode,
    /// Manual proxy URL (http://, https:// or socks5://), used when mode = manual.
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    Off,
    /// Environment/system proxy detection (on Linux this reads env vars only).
    #[default]
    System,
    Manual,
}
