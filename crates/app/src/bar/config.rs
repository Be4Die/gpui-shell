use serde::{Deserialize, Serialize};

use super::modules::{
    ActiveWindowConfig, BatteryConfig, ClockConfig, KeyboardLayoutConfig, LauncherBtnConfig,
    MprisConfig, SettingsConfig, SysInfoConfig, TrayConfig, WorkspacesConfig,
};

/// Bar screen position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarPosition {
    /// Left edge of the screen.
    #[default]
    Left,
    /// Right edge of the screen.
    Right,
    /// Top edge of the screen.
    Top,
    /// Bottom edge of the screen.
    Bottom,
}

impl BarPosition {
    #[inline(always)]
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// The same edge, as the UI layer names it. Config keeps its own enum
    /// because it carries the serde spelling.
    pub fn edge(self) -> ui::patterns::BarEdge {
        use ui::patterns::BarEdge;
        match self {
            Self::Left => BarEdge::Left,
            Self::Right => BarEdge::Right,
            Self::Top => BarEdge::Top,
            Self::Bottom => BarEdge::Bottom,
        }
    }
}

/// How the bar meets the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarStyle {
    /// Flush against the screen edge.
    #[default]
    Attached,
    /// Inset from the screen edges with rounded corners (see `bar.floating`).
    Floating,
    /// One side of a frame around the screen with rounded inner corners
    /// (see `bar.frame`).
    Frame,
    /// A transparent bar where each entry of `start`, `center` and `end` is
    /// its own pill. A nested list (`["Clock", "Notifications"]`) shares one.
    Bubbles,
}

/// Geometry of a floating bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarFloatingConfig {
    /// Distance in px between the bar and the screen edges.
    pub gap: f32,
    /// Corner radius in px.
    pub radius: f32,
}

impl Default for BarFloatingConfig {
    fn default() -> Self {
        Self {
            gap: 8.0,
            radius: 12.0,
        }
    }
}

/// Geometry of the screen frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarFrameConfig {
    /// Width in px of the three frame edges the bar is not on. Windows are
    /// kept inside them.
    pub thickness: f32,
    /// Radius in px of the frame's inner corners.
    pub radius: f32,
}

impl Default for BarFrameConfig {
    fn default() -> Self {
        Self {
            thickness: 6.0,
            radius: 16.0,
        }
    }
}

/// A section entry: one widget, or several sharing a bubble.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BarItem {
    Widget(String),
    Group(Vec<String>),
}

impl BarItem {
    pub fn names(&self) -> &[String] {
        match self {
            Self::Widget(name) => std::slice::from_ref(name),
            Self::Group(names) => names,
        }
    }
}

impl From<&str> for BarItem {
    fn from(name: &str) -> Self {
        Self::Widget(name.to_string())
    }
}

/// Status bar configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    /// Main axis thickness in px (height for horizontal, width for vertical).
    pub size: f32,
    /// Horizontal outer padding applied to the bar contents.
    pub padding: f32,
    /// Screen edge where the bar is placed.
    pub position: BarPosition,
    /// `attached`, `floating`, `frame` or `bubbles`.
    pub style: BarStyle,
    /// Opacity of the bar (and frame) background, from 0.0 (transparent) to 1.0.
    pub background: f32,
    /// Used when `style = "floating"`.
    pub floating: BarFloatingConfig,
    /// Used when `style = "frame"`.
    pub frame: BarFrameConfig,
    /// Whether the bar draws a border: the screen-facing edge when attached,
    /// all around when floating, the inner outline when framed, around each
    /// pill with bubbles.
    pub show_border: bool,
    /// Whether widgets render a resting background fill.
    pub widget_background: bool,
    /// Whether widgets render a subtle border.
    pub widget_border: bool,
    /// Start section widgets (left for horizontal, top for vertical).
    pub start: Vec<BarItem>,
    /// Center section widgets.
    pub center: Vec<BarItem>,
    /// End section widgets (right for horizontal, bottom for vertical).
    pub end: Vec<BarItem>,
    /// Bar modules
    pub modules: ModulesConfig,
}

/// Bar module configurations.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModulesConfig {
    pub clock: ClockConfig,
    pub battery: BatteryConfig,
    pub workspaces: WorkspacesConfig,
    pub tray: TrayConfig,
    pub sysinfo: SysInfoConfig,
    pub mpris: MprisConfig,
    pub active_window: ActiveWindowConfig,
    pub keyboard_layout: KeyboardLayoutConfig,
    pub launcher_btn: LauncherBtnConfig,
    pub settings: SettingsConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            size: 32.0,
            padding: 14.0,
            position: BarPosition::Left,
            style: BarStyle::default(),
            background: 1.0,
            floating: BarFloatingConfig::default(),
            frame: BarFrameConfig::default(),
            show_border: true,
            widget_background: true,
            widget_border: true,
            start: vec!["LauncherBtn".into(), "Workspaces".into(), "SysInfo".into()],
            center: vec!["ActiveWindow".into()],
            end: vec![
                "Clock".into(),
                "Mpris".into(),
                "Notifications".into(),
                "Systray".into(),
                "KeyboardLayout".into(),
                "Settings".into(),
            ],
            modules: ModulesConfig::default(),
        }
    }
}

impl BarConfig {
    #[inline(always)]
    pub fn is_vertical(&self) -> bool {
        self.position.is_vertical()
    }

    /// Distance from the screen edge to the bar's far side: what the bar
    /// takes away from the rest of the output.
    pub fn depth(&self) -> f32 {
        match self.style {
            BarStyle::Floating => self.floating.gap + self.size,
            BarStyle::Attached | BarStyle::Frame | BarStyle::Bubbles => self.size,
        }
    }

    /// The bar background, after `background` opacity.
    pub fn background_color(&self, theme: &ui::Theme) -> gpui::Hsla {
        theme
            .colors()
            .background
            .opacity(self.background.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A section mixes plain names with nested lists, the bubbles that hold several widgets.
    #[test]
    fn sections_take_widgets_and_groups() {
        let raw = concat!(
            "style = \"bubbles\"\n",
            "start = [\"LauncherBtn\", [\"Clock\", \"Notifications\"]]\n",
        );

        let bar: BarConfig = toml::from_str(raw).expect("bar config parses");

        assert_eq!(bar.style, BarStyle::Bubbles);
        assert_eq!(bar.start[0], BarItem::from("LauncherBtn"));
        assert_eq!(
            bar.start[1].names(),
            ["Clock".to_string(), "Notifications".to_string()]
        );
    }

    #[test]
    fn a_floating_bar_takes_its_gap_from_the_output() {
        let bar = BarConfig {
            style: BarStyle::Floating,
            ..BarConfig::default()
        };
        assert_eq!(bar.depth(), bar.size + bar.floating.gap);
    }
}
