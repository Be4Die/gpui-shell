//! Dock configuration.

use serde::{Deserialize, Serialize};

use crate::bar::config::BarPosition;

/// Which monitors show a dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DockMonitors {
    /// Only the primary display gets a dock (default).
    #[default]
    PrimaryOnly,
    /// Every display gets its own dock, scoped to that monitor's windows.
    All,
}

impl DockMonitors {
    pub const ALL: [Self; 2] = [Self::PrimaryOnly, Self::All];

    pub fn label(self) -> &'static str {
        match self {
            Self::PrimaryOnly => "Primary only",
            Self::All => "All monitors",
        }
    }
}

/// When the dock hides itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DockVisibility {
    /// Never hides.
    AlwaysVisible,
    /// Hides after a short delay once no window on the dock's monitor is
    /// focused; reveals on pointer-enter at the dock's edge, or as soon as
    /// focus returns to this monitor.
    #[default]
    IntelligentHide,
    /// Hides only while a window on the dock's monitor's current workspace
    /// covers the dock; comes back when the pointer reaches the dock's edge
    /// or nothing is in the way anymore. On compositors that report no
    /// window geometry (Niri), any window on that workspace counts as
    /// covering it.
    DodgeWindows,
}

impl DockVisibility {
    pub const ALL: [Self; 3] = [
        Self::AlwaysVisible,
        Self::IntelligentHide,
        Self::DodgeWindows,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AlwaysVisible => "Always visible",
            Self::IntelligentHide => "Intelligent hide",
            Self::DodgeWindows => "Dodge windows",
        }
    }
}

/// Hover effect applied to a dock item under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DockHoverEffect {
    /// No hover effect.
    None,
    /// Icon rises under the cursor.
    #[default]
    Lift,
    /// Icon grows under the cursor.
    Magnify,
    /// Icon gets a themed accent-color ring under the cursor.
    Glow,
    /// Icon both grows and rises under the cursor.
    MagnifyLift,
}

impl DockHoverEffect {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Lift,
        Self::Magnify,
        Self::Glow,
        Self::MagnifyLift,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Lift => "Lift",
            Self::Magnify => "Magnify",
            Self::Glow => "Glow",
            Self::MagnifyLift => "Magnify + lift",
        }
    }
}

/// Dock configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DockConfig {
    /// Screen edge the dock is anchored to. Independent of `bar.position`.
    pub position: BarPosition,
    /// Which monitors get a dock.
    pub monitors: DockMonitors,
    /// When the dock hides itself.
    pub visibility: DockVisibility,
    /// Hover effect applied to dock items.
    pub hover_effect: DockHoverEffect,
    /// Icon size in px.
    pub icon_size: f32,
    /// Pinned app identifiers (desktop file names, e.g. `"firefox.desktop"`).
    /// Pinning from the UI writes `state.toml`, which then overrides this.
    pub pinned: Vec<String>,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            position: BarPosition::Bottom,
            monitors: DockMonitors::default(),
            visibility: DockVisibility::default(),
            hover_effect: DockHoverEffect::default(),
            icon_size: 40.0,
            pinned: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DockConfig, DockHoverEffect, DockVisibility};

    #[test]
    fn hover_effect_deserializes_magnify_lift_from_kebab_case() {
        let config: DockConfig = toml::from_str("hover_effect = \"magnify-lift\"").unwrap();

        assert_eq!(config.hover_effect, DockHoverEffect::MagnifyLift);
    }

    #[test]
    fn visibility_defaults_to_intelligent_hide() {
        let config: DockConfig = toml::from_str("").unwrap();

        assert_eq!(config.visibility, DockVisibility::IntelligentHide);
        let explicit: DockConfig = toml::from_str("visibility = \"always-visible\"").unwrap();
        assert_eq!(explicit.visibility, DockVisibility::AlwaysVisible);
    }
}
