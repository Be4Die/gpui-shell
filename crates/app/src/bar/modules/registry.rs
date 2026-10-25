//! Widget registry for dynamic widget creation.
//!
//! This module provides a registry pattern that allows widgets to be created
//! by name, enabling configuration-driven bar layouts.

use gpui::{AnyElement, Context, Entity, prelude::*};
use ui::IconName;

use crate::notification::NotificationWidget;

use super::{
    ActiveWindow, Battery, Clock, KeyboardLayout, LauncherBtn, Mpris, Settings, SysInfo, Tray,
    Workspaces,
};

/// Every bar widget the config can name: the catalogue a config editor
/// offers, and the parser `Widget::create` goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    ActiveWindow,
    Clock,
    Battery,
    Workspaces,
    KeyboardLayout,
    Tray,
    SysInfo,
    LauncherBtn,
    Mpris,
    Notification,
    Settings,
}

impl WidgetKind {
    /// Display order, roughly the default bar from start to end.
    pub const ALL: [WidgetKind; 11] = [
        WidgetKind::LauncherBtn,
        WidgetKind::Workspaces,
        WidgetKind::SysInfo,
        WidgetKind::ActiveWindow,
        WidgetKind::Clock,
        WidgetKind::Battery,
        WidgetKind::Mpris,
        WidgetKind::Notification,
        WidgetKind::Tray,
        WidgetKind::KeyboardLayout,
        WidgetKind::Settings,
    ];

    /// Every spelling `create` ever accepted, so old configs keep working.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "ActiveWindow" | "WindowTitle" => Self::ActiveWindow,
            "Clock" => Self::Clock,
            "Battery" => Self::Battery,
            "Workspaces" => Self::Workspaces,
            "KeyboardLayout" => Self::KeyboardLayout,
            "Systray" | "Tray" => Self::Tray,
            "SysInfo" => Self::SysInfo,
            "LauncherBtn" | "Launcher" => Self::LauncherBtn,
            "Mpris" | "Media" | "Player" => Self::Mpris,
            "Notification" | "Notifications" => Self::Notification,
            "Settings" | "Info" | "ControlCenter" => Self::Settings,
            _ => return None,
        })
    }

    /// The spelling the default config uses.
    pub fn name(self) -> &'static str {
        match self {
            Self::ActiveWindow => "ActiveWindow",
            Self::Clock => "Clock",
            Self::Battery => "Battery",
            Self::Workspaces => "Workspaces",
            Self::KeyboardLayout => "KeyboardLayout",
            Self::Tray => "Systray",
            Self::SysInfo => "SysInfo",
            Self::LauncherBtn => "LauncherBtn",
            Self::Mpris => "Mpris",
            Self::Notification => "Notifications",
            Self::Settings => "Settings",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ActiveWindow => "Active window",
            Self::Clock => "Clock",
            Self::Battery => "Battery",
            Self::Workspaces => "Workspaces",
            Self::KeyboardLayout => "Keyboard layout",
            Self::Tray => "System tray",
            Self::SysInfo => "System info",
            Self::LauncherBtn => "Launcher button",
            Self::Mpris => "Media",
            Self::Notification => "Notifications",
            Self::Settings => "Settings",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ActiveWindow => "Title of the focused window.",
            Self::Clock => "Current time and date.",
            Self::Battery => "Charge level and charging state.",
            Self::Workspaces => "Workspace switcher for the compositor.",
            Self::KeyboardLayout => "Active keyboard layout.",
            Self::Tray => "Status notifier icons from running apps.",
            Self::SysInfo => "CPU, memory and temperature readouts.",
            Self::LauncherBtn => "Opens the command launcher.",
            Self::Mpris => "Now-playing controls for media players.",
            Self::Notification => "Unread count and the notification center.",
            Self::Settings => "Opens the control center.",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::ActiveWindow => IconName::Layout,
            Self::Clock => IconName::Clock,
            Self::Battery => IconName::Battery,
            Self::Workspaces => IconName::Layers,
            Self::KeyboardLayout => IconName::Keyboard,
            Self::Tray => IconName::Inbox,
            Self::SysInfo => IconName::Cpu,
            Self::LauncherBtn => IconName::MagnifyingGlass,
            Self::Mpris => IconName::Music,
            Self::Notification => IconName::Bell,
            Self::Settings => IconName::Settings,
        }
    }

    fn build<V: 'static>(self, cx: &mut Context<V>) -> Widget {
        match self {
            Self::ActiveWindow => Widget::ActiveWindow(cx.new(ActiveWindow::new)),
            Self::Clock => Widget::Clock(cx.new(Clock::new)),
            Self::Battery => Widget::Battery(cx.new(Battery::new)),
            Self::Workspaces => Widget::Workspaces(cx.new(Workspaces::new)),
            Self::KeyboardLayout => Widget::KeyboardLayout(cx.new(KeyboardLayout::new)),
            Self::Tray => Widget::Tray(cx.new(Tray::new)),
            Self::SysInfo => Widget::SysInfo(cx.new(SysInfo::new)),
            Self::LauncherBtn => Widget::LauncherBtn(cx.new(LauncherBtn::new)),
            Self::Mpris => Widget::Mpris(cx.new(Mpris::new)),
            Self::Notification => Widget::Notification(cx.new(NotificationWidget::new)),
            Self::Settings => Widget::Settings(cx.new(Settings::new)),
        }
    }
}

/// Wrapper enum for all possible widget types.
///
/// Each variant holds an Entity handle to a specific widget type.
/// This allows heterogeneous widgets to be stored in collections
/// and rendered uniformly.
pub enum Widget {
    ActiveWindow(Entity<ActiveWindow>),
    Clock(Entity<Clock>),
    Battery(Entity<Battery>),
    Workspaces(Entity<Workspaces>),
    KeyboardLayout(Entity<KeyboardLayout>),
    Tray(Entity<Tray>),
    SysInfo(Entity<SysInfo>),
    LauncherBtn(Entity<LauncherBtn>),
    Notification(Entity<NotificationWidget>),
    Settings(Entity<Settings>),
    Mpris(Entity<Mpris>),
}

impl Widget {
    /// Render the widget to an AnyElement.
    ///
    /// This allows uniform rendering regardless of the underlying widget type.
    pub fn render(&self) -> AnyElement {
        match self {
            Widget::ActiveWindow(e) => e.clone().into_any_element(),
            Widget::Clock(e) => e.clone().into_any_element(),
            Widget::Battery(e) => e.clone().into_any_element(),
            Widget::Workspaces(e) => e.clone().into_any_element(),
            Widget::KeyboardLayout(e) => e.clone().into_any_element(),
            Widget::Tray(e) => e.clone().into_any_element(),
            Widget::SysInfo(e) => e.clone().into_any_element(),
            Widget::LauncherBtn(e) => e.clone().into_any_element(),
            Widget::Notification(e) => e.clone().into_any_element(),
            Widget::Settings(e) => e.clone().into_any_element(),
            Widget::Mpris(e) => e.clone().into_any_element(),
        }
    }

    /// Create a widget by name.
    ///
    /// Returns `None` if the widget name is unknown.
    pub fn create<V: 'static>(name: &str, cx: &mut Context<V>) -> Option<Widget> {
        let kind = WidgetKind::parse(name);
        if kind.is_none() {
            tracing::warn!("Unknown widget: {}", name);
        }
        kind.map(|kind| kind.build(cx))
    }

    /// Create multiple widgets from a config list.
    pub fn create_many<V: 'static>(widgets: &[String], cx: &mut Context<V>) -> Vec<Widget> {
        widgets
            .iter()
            .filter_map(|widget| Widget::create(widget, cx))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::WidgetKind;

    #[test]
    fn every_alias_parses_to_its_kind() {
        let aliases = [
            ("ActiveWindow", WidgetKind::ActiveWindow),
            ("WindowTitle", WidgetKind::ActiveWindow),
            ("Clock", WidgetKind::Clock),
            ("Battery", WidgetKind::Battery),
            ("Workspaces", WidgetKind::Workspaces),
            ("KeyboardLayout", WidgetKind::KeyboardLayout),
            ("Systray", WidgetKind::Tray),
            ("Tray", WidgetKind::Tray),
            ("SysInfo", WidgetKind::SysInfo),
            ("LauncherBtn", WidgetKind::LauncherBtn),
            ("Launcher", WidgetKind::LauncherBtn),
            ("Mpris", WidgetKind::Mpris),
            ("Media", WidgetKind::Mpris),
            ("Player", WidgetKind::Mpris),
            ("Notification", WidgetKind::Notification),
            ("Notifications", WidgetKind::Notification),
            ("Settings", WidgetKind::Settings),
            ("Info", WidgetKind::Settings),
            ("ControlCenter", WidgetKind::Settings),
        ];
        for (name, kind) in aliases {
            assert_eq!(WidgetKind::parse(name), Some(kind), "{name}");
        }
        assert_eq!(WidgetKind::parse("Nope"), None);
    }

    #[test]
    fn canonical_names_round_trip() {
        for kind in WidgetKind::ALL {
            assert_eq!(WidgetKind::parse(kind.name()), Some(kind), "{kind:?}");
            assert!(!kind.label().is_empty());
            assert!(!kind.description().is_empty());
        }
    }

    #[test]
    fn default_bar_names_parse() {
        let bar = crate::bar::config::BarConfig::default();
        for item in bar.start.iter().chain(&bar.center).chain(&bar.end) {
            for name in item.names() {
                assert!(WidgetKind::parse(name).is_some(), "{name}");
            }
        }
    }
}
