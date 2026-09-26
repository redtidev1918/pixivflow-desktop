//! Desktop-side `config.json` — user editable, lives in the OS config dir.
//! Field semantics are documented in the README. Empty string fields mean
//! "use the desktop default" (resolved at runtime against app dirs).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Top-level desktop config, serialized as camelCase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    /// "local" = spawn/use a local PixivFlow backend and load its bundled UI.
    /// "remote" = load an existing remote PixivFlow WebUI URL; no local backend.
    pub mode: String,
    /// Base URL (scheme://host:port) used when `mode == "remote"`.
    pub remote_url: String,
    pub backend: BackendConfig,
    /// Where PixivFlow keeps its data (sqlite db + generated files).
    /// Empty => `<app_data_dir>/data`.
    pub data_dir: String,
    /// Where downloads are written. Empty => `<data_dir>/downloads`.
    pub download_dir: String,
    /// Where the desktop app keeps its own log file.
    /// Empty => `<app_data_dir>/logs`.
    pub logs_dir: String,
}

/// Backend sub-config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BackendConfig {
    /// Auto-start the backend when the app launches (local mode).
    pub auto_start: bool,
    /// Bind host. Keep loopback unless you enable backend auth. `localhost` is
    /// normalized to `127.0.0.1` so loopback stays credential-free.
    pub host: String,
    /// WebUI/API port. Default 3000 (PixivFlow PROD_API).
    pub port: u16,
    /// Path to the `node` executable. Empty => system `node`.
    pub node_path: String,
    /// Path to the PixivFlow backend root (the dir containing `dist/webui/index.js`).
    /// Empty => auto-detect (bundled resource, then `PIXIVFLOW_DIST_PATH` env).
    pub dist_path: String,
    /// Optional full launch program override (e.g. a packaged Windows
    /// executable) that bypasses `node + dist entry`. Empty => normal node path.
    pub command: String,
    /// How long to wait for `/api/health` to become OK before declaring failure.
    pub launch_timeout_ms: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            mode: "local".to_string(),
            remote_url: String::new(),
            backend: BackendConfig::default(),
            data_dir: String::new(),
            download_dir: String::new(),
            logs_dir: String::new(),
        }
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        BackendConfig {
            auto_start: true,
            host: "127.0.0.1".to_string(),
            port: 3000,
            node_path: String::new(),
            dist_path: String::new(),
            command: String::new(),
            launch_timeout_ms: 60_000,
        }
    }
}

/// Loads (creating with defaults if missing) `config.json` in `config_dir`.
/// Returns `(config, path_to_file)`.
pub fn load_config(config_dir: &Path) -> std::io::Result<(AppConfig, PathBuf)> {
    fs::create_dir_all(config_dir)?;
    let path = config_dir.join("config.json");
    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let cfg: AppConfig = serde_json::from_str(&raw).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("config.json is invalid JSON: {e}"),
            )
        })?;
        Ok((cfg, path))
    } else {
        let cfg = AppConfig::default();
        let _ = save_config(&cfg, &path);
        Ok((cfg, path))
    }
}

/// Writes `config.json` atomically (temp file + rename).
pub fn save_config(cfg: &AppConfig, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    fs::write(&tmp, raw)?;
    fs::rename(&tmp, path)?;
    Ok(())
}