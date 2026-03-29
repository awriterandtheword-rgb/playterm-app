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
    #[serde(default)]
    pub keybinds: KeybindsSection,
    #[serde(default)]
    pub theme: ThemeSection,
    #[serde(default)]
    pub ui: UiSection,
    #[serde(default)]
    pub cache: CacheSection,
}

// ── [keybinds] ────────────────────────────────────────────────────────────────

/// Raw keybind strings from config.toml. Every field is `Option<String>`;
/// unset fields fall back to built-in defaults inside `Keybinds::from_section`.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct KeybindsSection {
    pub scroll_up:     Option<String>,
    pub scroll_down:   Option<String>,
    pub column_left:   Option<String>,
    pub column_right:  Option<String>,
    pub play_pause:    Option<String>,
    pub next_track:    Option<String>,
    pub prev_track:    Option<String>,
    pub seek_forward:  Option<String>,
    pub seek_backward: Option<String>,
    pub add_track:     Option<String>,
    pub add_all:       Option<String>,
    pub shuffle:       Option<String>,
    pub unshuffle:     Option<String>,
    pub clear_queue:   Option<String>,
    pub search:        Option<String>,
    pub volume_up:     Option<String>,
    pub volume_down:   Option<String>,
    pub tab_switch:         Option<String>,
    /// Reverse tab cycle (Backtick by default)
    pub tab_switch_reverse: Option<String>,
    /// Jump to Home tab (default: '1')
    pub go_to_home:         Option<String>,
    /// Jump to Browser tab (default: '2')
    pub go_to_browser:      Option<String>,
    /// Jump to NowPlaying tab (default: '3')
    pub go_to_nowplaying:   Option<String>,
    pub quit:               Option<String>,
}

// ── [theme] ───────────────────────────────────────────────────────────────────

// ── [ui] ─────────────────────────────────────────────────────────────────────

// ── [cache] ───────────────────────────────────────────────────────────────────

/// Offline track cache settings from config.toml.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheSection {
    /// Whether the track cache is enabled. Default: true.
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
    /// Maximum total cache size in gigabytes. Default: 2.0.
    #[serde(default = "default_cache_max_size_gb")]
    pub max_size_gb: f64,
}

fn default_cache_enabled() -> bool { true }
fn default_cache_max_size_gb() -> f64 { 2.0 }

impl Default for CacheSection {
    fn default() -> Self {
        Self { enabled: default_cache_enabled(), max_size_gb: default_cache_max_size_gb() }
    }
}

/// UI preferences from config.toml.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct UiSection {
    /// Show the lyrics overlay on the NowPlaying tab by default. Default: false.
    #[serde(default)]
    pub lyrics: bool,
}

/// Raw hex colour strings from config.toml. Defaults inside `Theme::from_section`.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ThemeSection {
    pub accent:        Option<String>,
    pub background:    Option<String>,
    pub surface:       Option<String>,
    pub foreground:    Option<String>,
    pub dimmed:        Option<String>,
    pub border:        Option<String>,
    pub border_active: Option<String>,
    /// Whether to extract and apply a dynamic accent colour from album art.
    /// Default: true.
    pub dynamic:       Option<bool>,
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
    pub subsonic_url:   String,
    pub subsonic_user:  String,
    pub subsonic_pass:  String,
    pub default_volume: u8,
    pub max_bit_rate:   u32,
    /// Raw keybind strings — parsed into `Keybinds` by `App::new`.
    pub keybinds: KeybindsSection,
    /// Raw theme colour strings — parsed into `Theme` by `App::new`.
    pub theme:    ThemeSection,
    /// Whether to show the lyrics overlay on startup.
    pub lyrics_visible: bool,
    /// Whether the offline track cache is enabled.
    pub cache_enabled:     bool,
    /// Maximum total cache size in gigabytes.
    pub cache_max_size_gb: f64,
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
            subsonic_url:      file_cfg.server.url,
            subsonic_user:     file_cfg.server.username,
            subsonic_pass:     file_cfg.server.password,
            default_volume:    file_cfg.player.default_volume,
            max_bit_rate:      file_cfg.player.max_bit_rate,
            keybinds:          file_cfg.keybinds,
            theme:             file_cfg.theme,
            lyrics_visible:    file_cfg.ui.lyrics,
            cache_enabled:     file_cfg.cache.enabled,
            cache_max_size_gb: file_cfg.cache.max_size_gb,
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
    let default_toml = r##"[server]
url = ""
username = ""
password = ""

[player]
default_volume = 70
max_bit_rate = 0   # 0 = unlimited; set e.g. 320 to cap streaming bitrate

[keybinds]
# scroll_up     = "k"
# scroll_down   = "j"
# column_left   = "h"
# column_right  = "l"
# play_pause    = "p"
# next_track    = "n"
# prev_track    = "N"
# seek_forward  = "Right"
# seek_backward = "Left"
# add_track     = "a"
# add_all       = "Shift+a"
# shuffle       = "x"
# unshuffle     = "z"
# clear_queue   = "D"
# search        = "/"
# volume_up     = "+"
# volume_down   = "-"
# tab_switch    = "Tab"
# quit          = "q"

[theme]
# accent        = "#ff8c00"   # highlighted items, active borders, progress fill
# background    = "#1a1a1a"   # outer background (status bar, now-playing bar)
# surface       = "#161616"   # panel backgrounds (browser columns, queue)
# foreground    = "#d4d0c8"   # primary text
# dimmed        = "#5a5858"   # muted / secondary text
# border        = "#252525"   # inactive pane borders
# border_active = "#3a3a3a"   # active pane borders
# dynamic       = true         # extract accent colour from album art

[ui]
lyrics = false   # show lyrics overlay on NowPlaying tab (toggle with L)

[cache]
enabled     = true
max_size_gb = 2   # maximum total cache size in gigabytes
"##;
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
