use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

// ── File-level serde structs ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
struct FileConfig {
    #[serde(default)]
    server: ServerSection,
    #[serde(default)]
    player: PlayerSection,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ServerSection {
    #[serde(default)]
    url: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlayerSection {
    #[serde(default = "default_volume")]
    default_volume: u8,
    #[serde(default)]
    max_bit_rate: u32,
}

impl Default for PlayerSection {
    fn default() -> Self {
        Self { default_volume: default_volume(), max_bit_rate: 0 }
    }
}

fn default_volume() -> u8 { 70 }

// ── Runtime config ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Config {
    pub subsonic_url: String,
    pub subsonic_user: String,
    pub subsonic_pass: String,
    pub default_volume: u8,
    pub max_bit_rate: u32,
}

impl Config {
    /// Load config from `~/.config/playterm/config.toml`, creating a default
    /// file if it doesn't exist. Env vars override file values.
    /// Returns an error (with message) if no password is configured.
    pub fn load() -> Result<Self> {
        let config_path = config_file_path()?;

        // Create default file if missing.
        if !config_path.exists() {
            create_default(&config_path)?;
        }

        let text = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        let mut file_cfg: FileConfig = toml::from_str(&text)
            .with_context(|| format!("parsing {}", config_path.display()))?;

        // Env vars override file values.
        merge_env_overrides(&mut file_cfg);

        // Validate password.
        if file_cfg.server.password.is_empty() {
            bail!(
                "No Subsonic password configured.\n\
                 Edit {} or set TERMUSIC_SUBSONIC_PASS.",
                config_path.display()
            );
        }

        Ok(Config {
            subsonic_url: file_cfg.server.url,
            subsonic_user: file_cfg.server.username,
            subsonic_pass: file_cfg.server.password,
            default_volume: file_cfg.player.default_volume,
            max_bit_rate: file_cfg.player.max_bit_rate,
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("playterm"));
    }
    let home = std::env::var("HOME").context("HOME env var not set")?;
    Ok(PathBuf::from(home).join(".config").join("playterm"))
}

fn config_file_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

fn create_default(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating config dir {}", parent.display()))?;
    }
    let default_toml = r#"[server]
url = ""
username = ""
password = ""

[player]
default_volume = 70
max_bit_rate = 0
"#;
    std::fs::write(path, default_toml)
        .with_context(|| format!("writing default config to {}", path.display()))?;
    eprintln!("Created default config: {}", path.display());
    Ok(())
}

fn merge_env_overrides(cfg: &mut FileConfig) {
    if let Ok(v) = std::env::var("TERMUSIC_SUBSONIC_URL").or_else(|_| std::env::var("SUBSONIC_URL")) {
        cfg.server.url = v;
    }
    if let Ok(v) = std::env::var("TERMUSIC_SUBSONIC_USER").or_else(|_| std::env::var("SUBSONIC_USER")) {
        cfg.server.username = v;
    }
    if let Ok(v) = std::env::var("TERMUSIC_SUBSONIC_PASS").or_else(|_| std::env::var("SUBSONIC_PASS")) {
        cfg.server.password = v;
    }
}
