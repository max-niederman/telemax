use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppShortcut {
    pub id: String,
    pub name: String,
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_sensitivity")]
    pub trackpad_sensitivity: f64,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub app_shortcuts: Vec<AppShortcut>,
    #[serde(default = "default_visible_actions")]
    pub visible_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_device: Option<String>,
}

fn default_sensitivity() -> f64 {
    1.0
}

fn default_visible_actions() -> Vec<String> {
    vec![
        "close-window".into(),
        "fullscreen-window".into(),
        "maximize-column".into(),
        "screenshot".into(),
        "power-off-monitors".into(),
        "power-on-monitors".into(),
    ]
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            trackpad_sensitivity: default_sensitivity(),
            theme: Theme::default(),
            app_shortcuts: Vec::new(),
            visible_actions: default_visible_actions(),
            audio_device: None,
        }
    }
}

pub type SharedSettings = Arc<RwLock<Settings>>;

fn config_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("telemax").join("settings.json")
}

pub fn load() -> Settings {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            tracing::warn!("invalid settings file at {}: {e}, using defaults", path.display());
            Settings::default()
        }),
        Err(_) => {
            tracing::info!("no settings file at {}, using defaults", path.display());
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, json)
}
