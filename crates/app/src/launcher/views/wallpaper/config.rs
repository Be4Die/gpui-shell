//! Wallpaper view configuration.

use serde::{Deserialize, Serialize};

/// Wallpaper view configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WallpaperConfig {
    pub prefix: String,
    pub directory: String,
    /// Cards per grid row.
    pub columns: usize,
    /// Generate a theme from the applied wallpaper. The toggle in the view
    /// overrides this, and is remembered in `state.toml`.
    pub matugen: bool,
    /// Generate a dark theme rather than a light one. Overridden like
    /// `matugen`.
    pub dark_mode: bool,
    /// matugen `--type`, e.g. `scheme-tonal-spot`, `scheme-content`.
    pub matugen_scheme: String,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            prefix: ";wp".into(),
            directory: "~/Pictures/Wallpapers".into(),
            columns: 3,
            matugen: true,
            dark_mode: true,
            matugen_scheme: "scheme-tonal-spot".into(),
        }
    }
}
