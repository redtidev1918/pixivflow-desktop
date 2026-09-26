//! Configuration system (F1).
//!
//! Reads `desktop-config.json` from the platform app-config dir and auto-creates
//! a default file when missing. Malformed JSON surfaces as a config error which
//! the frontend displays.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

/// Backend process override (F2.1 adapter). `command` is a plain executable
/// path or a bare command resolved via PATH. Empty by default: F2 points this
/// at the real PixivFlow so we never assume a fixed path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackendConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}
impl Default for BackendConfig {
    fn default() -> Self {
        Self { port: default_port(), auto_start: default_true(), command: String::new(), args: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteConfig {
    #[serde(default)]
    pub url: String,
}
impl Default for RemoteConfig {
    fn default() -> Self {
        Self { url: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    /// `"local"` starts a local backend; `"remote"` points at an existing server.
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default)]
    pub download_dir: String,
    #[serde(default)]
    pub log_dir: String,
    #[serde(default)]
    pub remote: RemoteConfig,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            backend: BackendConfig::default(),
            data_dir: String::new(),
            download_dir: String::new(),
            log_dir: String::new(),
            remote: RemoteConfig::default(),
        }
    }
}

fn default_port() -> u16 { 3000 }
fn default_true() -> bool { true }
fn default_mode() -> String { "local".into() }

impl AppConfig {
    pub fn is_local(&self) -> bool { self.mode.eq_ignore_ascii_case("local") }
    pub fn is_remote(&self) -> bool { self.mode.eq_ignore_ascii_case("remote") }
    pub fn port(&self) -> u16 { self.backend.port }
}

const DEFAULT_CONFIG_JSON: &str = r#"{
  "mode": "local",
  "backend": { "port": 3000, "autoStart": true },
  "dataDir": "",
  "downloadDir": "",
  "logDir": "",
  "remote": { "url": "" }
}"#;

/// Resolve the config file path (platform app-config dir).
pub fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create config dir: {e}"))?;
    Ok(dir.join("desktop-config.json"))
}

/// Load the config, auto-creating the default file when it does not exist.
/// Returns the parsed config plus the path it came from.
pub fn load_or_create(app: &tauri::AppHandle) -> Result<(AppConfig, PathBuf), String> {
    let path = config_path(app)?;
    if !path.exists() {
        // Dev convenience: honour a desktop-config.json next to the working dir.
        if let Ok(cwd) = std::env::current_dir() {
            let local = cwd.join("desktop-config.json");
            if local.exists() {
                let cfg = parse_file(&local)?;
                return Ok((cfg, local));
            }
        }
        std::fs::write(&path, DEFAULT_CONFIG_JSON)
            .map_err(|e| format!("create default config: {e}"))?;
        return Ok((AppConfig::default(), path));
    }
    let cfg = parse_file(&path)?;
    Ok((cfg, path))
}

fn parse_file(path: &PathBuf) -> Result<AppConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read config {path:?}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| {
        format!(
            "配置解析失败: {e}\n请检查配置文件格式: {}",
            path.display()
        )
    })
}
