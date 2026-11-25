use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tab_width: usize,
    pub line_numbers: bool,
    pub theme: String,
    pub tab_size: usize,
    /// Key bindings. Keys accept multi-key chords such as
    /// `"Ctrl-x Ctrl-s": "Save"`.
    pub keymap: std::collections::HashMap<String, String>,
    /// Mouse bindings, e.g. `{"MouseWheelUp": "PageUp"}`.
    #[serde(default)]
    pub mouse: std::collections::HashMap<String, String>,
    /// Per-plugin enable flags, keyed by script file stem. Absent means enabled,
    /// matching official micro's `pluginview` toggles.
    #[serde(default)]
    pub plugins: std::collections::HashMap<String, bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            line_numbers: true,
            theme: "base16-ocean.dark".into(),
            tab_size: 4,
            keymap: std::collections::HashMap::new(),
            mouse: std::collections::HashMap::new(),
            plugins: std::collections::HashMap::new(),
        }
    }
}

impl Config {
    /// Whether the plugin script named `stem` is enabled.
    pub fn plugin_enabled(&self, stem: &str) -> bool {
        self.plugins.get(stem).copied().unwrap_or(true)
    }

    pub fn path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("micro-rs");
        dir.join("config.json")
    }

    pub fn load_or_default() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
