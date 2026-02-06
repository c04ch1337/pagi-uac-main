//! Studio UI config: bundled default via include_str! (bare-metal, no external deps).

use serde::Deserialize;

/// Bundled default config so the app runs with no external files. Overridden by local file if present.
const DEFAULT_UI_CONFIG: &str = include_str!("../assets/ui_config.json");

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StudioConfig {
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default)]
    pub default_slot_id: u8,
    #[serde(default)]
    pub theme_dark: bool,
}

fn default_window_width() -> f32 {
    900.0
}
fn default_window_height() -> f32 {
    640.0
}

impl StudioConfig {
    /// Load config: local file (relative to manifest or current_dir) if present, else bundled default. No CDNs.
    pub fn load() -> Self {
        let manifest_assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let cwd_assets = std::env::current_dir()
            .ok()
            .map(|p| p.join("add-ons").join("pagi-studio-ui").join("assets"));

        let path = [manifest_assets, cwd_assets.unwrap_or_default()]
            .into_iter()
            .find(|b| b.join("ui_config.json").exists())
            .map(|b| b.join("ui_config.json"));

        let s = match path {
            Some(p) => std::fs::read_to_string(&p).ok(),
            None => None,
        };
        let s = s.unwrap_or_else(|| DEFAULT_UI_CONFIG.to_string());
        serde_json::from_str(&s).unwrap_or_default()
    }
}
