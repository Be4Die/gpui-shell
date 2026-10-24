//! WiFi section for the Control Center.
//!
//! Displays available networks with signal strength and connection status.
//! Supports connecting to open and protected networks with password input.

use gpui::{App, ElementId, MouseButton, SharedString, div, prelude::*};
use services::{AccessPoint, NetworkCommand};
use ui::{
    ActiveTheme, Color, Icon, IconName, IconSize, InputBuffer, Radius, Spacing, Spinner, TextSize,
    render_masked_input_line,
};

use crate::state::AppState;

use super::list;
use crate::icons;

/// State for WiFi password input
#[derive(Debug, Clone, Default)]
pub struct WifiPasswordState {
    /// The SSID we're trying to connect to
    pub ssid: Option<String>,
    /// The current password input
    pub input: InputBuffer,
    /// Whether we're currently connecting
    pub connecting: bool,
    /// Error message if connection failed
    pub error: Option<String>,
}

impl WifiPasswordState {
    /// Start password entry for a network
    pub fn start_for(&mut self, ssid: String) {
        self.ssid = Some(ssid);
        self.input.clear();
        self.connecting = false;
        self.error = None;
    }

    /// Clear the password state
    pub fn clear(&mut self) {
        self.ssid = None;
        self.input.clear();
        self.connecting = false;
        self.error = None;
    }

    /// Check if we're entering a password for a specific SSID
    pub fn is_entering_for(&self, ssid: &str) -> bool {
        self.ssid.as_deref() == Some(ssid)
    }
}

/// What a click in the WiFi section asks the control center to do.
pub enum WifiAction {
    /// Ask for a password before connecting.
    Prompt(String),
    Connect {
        ssid: String,
        password: Option<String>,
    },
    Disconnect(String),
    CancelPassword,
}

/// Render the WiFi section (network list)
pub fn render_wifi_section(
    password_state: &WifiPasswordState,
    on_action: impl Fn(WifiAction, &mut App) + Clone + 'static,
    cx: &App,
) -> impl IntoElement {
    let network = AppState::network(cx).get();

    // Get current connected WiFi SSID
    let connected_name = network.active_connections.iter().find_map(|c| {
        if let services::ActiveConnectionInfo::WiFi { name, .. } = c {
            Some(name.clone())
        } else {
            None
        }
    });
    let wifi_enabled = network.wifi_enabled;
    let scanning = network.scanning;

    // Sort access points: connected first, then by signal strength
    let mut aps: Vec<AccessPoint> = network.wireless_access_points.clone();
    aps.sort_by(|a, b| {
        let a_connected = connected_name.as_ref() == Some(&a.ssid);
        let b_connected = connected_name.as_ref() == Some(&b.ssid);
        match (a_connected, b_connected) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.strength.cmp(&a.strength),
        }
    });

    list::section(
        IconName::Wifi,
        "WiFi",
        |header| {
            header
                .when_some(connected_name.clone(), |el, name| {
                    el.child(
                        div()
                            .text_size(TextSize::XSmall.rems())
                            .text_color(cx.theme().colors.text)
                            .child(format!("- {}", name)),
                    )
                })
                .child(render_refresh_button(scanning, cx))
        },
        cx,
    )
    .when(wifi_enabled && scanning, |el| {
        el.child(list::hint("Scanning...", cx))
    })
    .when(!wifi_enabled, |el| {
        el.child(list::empty_state("WiFi is off", cx))
    })
    .when(wifi_enabled && aps.is_empty(), |el| {
        el.child(list::empty_state("No networks found", cx))
    })
    .when(wifi_enabled && !aps.is_empty(), |el| {
        el.child(
            list::list("wifi-networks-list", cx).children(aps.into_iter().enumerate().map(
                |(idx, ap)| {
                    let on_action = on_action.clone();
                    if password_state.is_entering_for(&ap.ssid) {
                        render_password_input(idx, ap.ssid, password_state, on_action, cx)
                            .into_any_element()
                    } else {
                        let connected = connected_name.as_ref() == Some(&ap.ssid);
                        render_network_item(idx, ap, connected, on_action, cx).into_any_element()
                    }
                },
            )),
        )
    })
}

/// Render a single network item in the list
fn render_network_item(
    index: usize,
    ap: AccessPoint,
    connected: bool,
    on_action: impl Fn(WifiAction, &mut App) + Clone + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let secured = !ap.public;
    let known = ap.known;
    let text_muted = theme.colors.text_muted;
    let status_success = theme.colors.status.success;
    let lock_tooltip = if known {
        "Secured (saved)"
    } else {
        "Secured (password required)"
    };

    list::row(format!("wifi-{index}"), connected, cx)
        .on_mouse_down(MouseButton::Left, {
            let ssid = ap.ssid.clone();
            let on_action = on_action.clone();
            move |_, _, cx| {
                if connected {
                    return;
                }
                let ssid = ssid.clone();
                // Known networks connect with NM's saved credentials.
                on_action(
                    if secured && !known {
                        WifiAction::Prompt(ssid)
                    } else {
                        WifiAction::Connect {
                            ssid,
                            password: None,
                        }
                    },
                    cx,
                );
            }
        })
        .child(list::row_icon(
            format!("wifi-signal-{index}"),
            icons::wifi_signal_icon(ap.strength),
            connected,
            format!("Signal strength: {}%", ap.strength),
            cx,
        ))
        .child(list::row_label(ap.ssid.clone(), cx))
        .when(known && !connected, |el| {
            el.child(list::badge(
                format!("wifi-known-{index}"),
                IconName::Check,
                status_success,
                "Saved network",
            ))
        })
        .when(secured, |el| {
            el.child(list::badge(
                format!("wifi-lock-{index}"),
                IconName::Lock,
                if known { status_success } else { text_muted },
                lock_tooltip,
            ))
        })
        .when(!connected, |el| {
            el.child(list::badge(
                format!("wifi-connect-{index}"),
                IconName::ChevronRight,
                text_muted,
                "Connect",
            ))
        })
        .when(connected, |el| {
            let ssid = ap.ssid.clone();
            el.child(list::action_button(
                format!("wifi-disconnect-{index}"),
                IconName::Close,
                status_success,
                "Disconnect",
                move |cx| on_action(WifiAction::Disconnect(ssid.clone()), cx),
                cx,
            ))
        })
}

/// Render password input row for a network
fn render_password_input(
    index: usize,
    ssid: String,
    state: &WifiPasswordState,
    on_action: impl Fn(WifiAction, &mut App) + Clone + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let connecting = state.connecting;
    let password_for_submit = state.input.text().to_string();
    let password_line = render_masked_input_line(&state.input, "Type password...", '•', cx);

    // Pre-compute colors for closures
    let bg_tertiary = theme.colors.elevated_surface_background;
    let bg_primary = theme.colors.background;
    let accent_primary = theme.colors.accent;
    let accent_hover = theme.colors.accent;
    let text_primary = theme.colors.text;
    let text_muted = theme.colors.text_muted;
    let status_error = theme.colors.status.error;
    let on_cancel = on_action.clone();

    div()
        .id(ElementId::Name(SharedString::from(format!(
            "wifi-password-{}",
            index
        ))))
        .flex()
        .flex_col()
        .gap(Spacing::XSmall.pixels())
        .w_full()
        .px(Spacing::Medium.pixels())
        .py(Spacing::Medium.pixels())
        .bg(bg_tertiary)
        .rounded(Radius::Small.pixels())
        // Network name header
        .child(
            div()
                .flex()
                .items_center()
                .gap(Spacing::Medium.pixels())
                .child(
                    Icon::new(IconName::Lock)
                        .size(IconSize::XSmall)
                        .color(Color::Accent),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(TextSize::Small.rems())
                        .text_color(text_primary)
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(ssid.clone()),
                )
                .child(
                    div()
                        .id(format!("wifi-cancel-{}", index))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            on_cancel(WifiAction::CancelPassword, cx);
                        })
                        .child(
                            Icon::new(IconName::Close)
                                .size(IconSize::XSmall)
                                .color(Color::Muted),
                        ),
                ),
        )
        // Password input with keyboard support
        .child(
            div()
                .flex()
                .items_center()
                .gap(Spacing::Medium.pixels())
                .child(
                    div()
                        .flex_1()
                        .px(Spacing::Medium.pixels())
                        .py(Spacing::XSmall.pixels())
                        .bg(bg_primary)
                        .rounded(Radius::Small.pixels())
                        .border_1()
                        .border_color(accent_primary)
                        .child(
                            div()
                                .text_size(TextSize::Small.rems())
                                .text_color(text_primary)
                                .child(password_line),
                        ),
                )
                .child(
                    div()
                        .id(format!("wifi-connect-{}", index))
                        .px(Spacing::Large.pixels())
                        .py(Spacing::XSmall.pixels())
                        .bg(accent_primary)
                        .rounded(Radius::Small.pixels())
                        .cursor_pointer()
                        .hover(move |s| s.bg(accent_hover))
                        .when(connecting, |el| el.opacity(0.7))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            if !connecting {
                                on_action(
                                    WifiAction::Connect {
                                        ssid: ssid.clone(),
                                        password: Some(password_for_submit.clone()),
                                    },
                                    cx,
                                );
                            }
                        })
                        .child(
                            div()
                                .text_size(TextSize::Small.rems())
                                .text_color(bg_primary)
                                .child(if connecting {
                                    "Connecting..."
                                } else {
                                    "Connect"
                                }),
                        ),
                ),
        )
        // Keyboard hints
        .child(
            div()
                .text_size(TextSize::XSmall.rems())
                .text_color(text_muted)
                .child("Press Enter to connect, Escape to cancel"),
        )
        // Error message
        .when_some(state.error.clone(), |el, err| {
            el.child(
                div()
                    .text_size(TextSize::XSmall.rems())
                    .text_color(status_error)
                    .child(err),
            )
        })
}

/// Render a refresh button for rescanning networks. It spins, and ignores
/// clicks, while a scan is running.
fn render_refresh_button(scanning: bool, cx: &App) -> impl IntoElement {
    let services = AppState::network(cx).clone();
    let interactive_hover = cx.theme().colors.element_hover;

    list::header_button("wifi-refresh", cx)
        .when(scanning, |el| {
            el.child(
                Spinner::new()
                    .icon(IconName::Refresh)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
        })
        .when(!scanning, |el| {
            el.cursor_pointer()
                .hover(move |s| s.bg(interactive_hover))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    services.dispatch(NetworkCommand::RequestScan);
                })
                .child(
                    Icon::new(IconName::Refresh)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
        })
}
