//! A thumbnail screen showing the dock on its edge: its icons at the
//! configured size, one under the cursor with the hover effect, and how the
//! visibility mode hides it. Clicking an edge moves the dock there.

use gpui::{AnyElement, App, BoxShadow, Div, Pixels, div, point, prelude::*, px};
use ui::{ActiveTheme, Radius};

use super::bar_preview::bar_zone;
use super::preview::{frame, hit_zones, scale, screen, set_field};
use super::schema::ConfigPath;
use crate::bar::config::BarPosition;
use crate::dock::config::{DockConfig, DockHoverEffect, DockVisibility};

/// How far the dock floats off the screen edge.
const EDGE_GAP: f32 = 3.;
/// How far a lifted icon rises; a quarter of the real 8px lift is invisible.
const LIFT: f32 = 3.;
/// A hidden dock shows faintly so the edge it would appear on stays legible.
const HIDDEN_OPACITY: f32 = 0.45;
const ICONS: usize = 5;
/// The icon drawn under the cursor.
const HOVERED: usize = 2;

pub fn dock_preview(path: &ConfigPath<DockConfig>, cx: &App) -> AnyElement {
    let dock = path.read(cx).clone();
    let colors = cx.theme().colors();
    let position = dock.position;
    let icon = scale(dock.icon_size, 5., 14.);

    let mut screen = screen(cx);
    if dock.visibility == DockVisibility::DodgeWindows {
        screen = screen.child(covering_window(position, cx));
    }

    let icons = (0..ICONS).map(|ix| {
        let hovered = ix == HOVERED;
        let mut dot = div()
            .flex_none()
            .rounded(Radius::Full.pixels())
            .size(icon)
            .bg(colors.accent.opacity(if hovered { 1. } else { 0.7 }));
        if hovered {
            let effect = dock.hover_effect;
            if matches!(
                effect,
                DockHoverEffect::Magnify | DockHoverEffect::MagnifyLift
            ) {
                dot = dot.size(icon * 1.4);
            }
            if matches!(effect, DockHoverEffect::Lift | DockHoverEffect::MagnifyLift) {
                dot = dot.relative().map(|dot| match position {
                    BarPosition::Left => dot.left(px(LIFT)),
                    BarPosition::Right => dot.right(px(LIFT)),
                    BarPosition::Top => dot.top(px(LIFT)),
                    BarPosition::Bottom => dot.bottom(px(LIFT)),
                });
            }
            if effect == DockHoverEffect::Glow {
                dot = dot.shadow(vec![BoxShadow {
                    color: colors.accent.opacity(0.7),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(4.),
                    spread_radius: px(1.5),
                    inset: false,
                }]);
            }
        }
        dot
    });
    let strip = div()
        .flex()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(Radius::Small.pixels())
        .border_1()
        .border_color(colors.border)
        .bg(colors.element_background)
        .map(|strip| {
            if position.is_vertical() {
                strip.flex_col()
            } else {
                strip
            }
        })
        .children(icons);
    let dock_layer = along_edge(
        div().absolute().flex().justify_center(),
        position,
        px(EDGE_GAP),
    )
    .when(dock.visibility != DockVisibility::AlwaysVisible, |layer| {
        layer.opacity(HIDDEN_OPACITY)
    })
    .child(strip);
    screen = screen.child(dock_layer);

    let setter = path.clone();
    screen = hit_zones(
        screen,
        "dock-preview",
        &BarPosition::ALL,
        position,
        bar_zone,
        move |position, cx| {
            set_field(
                &setter,
                "position",
                position,
                |dock, p| dock.position = p,
                cx,
            );
        },
        cx,
    );

    frame(
        screen,
        format!(
            "{} · {} · {}",
            position.label(),
            dock.visibility.label(),
            dock.hover_effect.label()
        ),
    )
}

/// Stretch `layer` along `position`'s edge, `offset` in from it, so a
/// centred child sits mid-edge.
fn along_edge(layer: Div, position: BarPosition, offset: Pixels) -> Div {
    match position {
        BarPosition::Left => layer.top_0().bottom_0().left(offset).flex_col(),
        BarPosition::Right => layer.top_0().bottom_0().right(offset).flex_col(),
        BarPosition::Top => layer.left_0().right_0().top(offset),
        BarPosition::Bottom => layer.left_0().right_0().bottom(offset),
    }
}

/// A window reaching over the dock's edge: the reason a dodging dock hides.
fn covering_window(position: BarPosition, cx: &App) -> impl IntoElement {
    let colors = cx.theme().colors();
    let far = px(22.);
    let near = px(2.);
    div()
        .absolute()
        .rounded(Radius::Small.pixels())
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface_background)
        .map(|window| match position {
            BarPosition::Left => window.top(far).bottom(far).right(far).left(near),
            BarPosition::Right => window.top(far).bottom(far).left(far).right(near),
            BarPosition::Top => window.left(far).right(far).bottom(far).top(near),
            BarPosition::Bottom => window.left(far).right(far).top(far).bottom(near),
        })
        .child(
            div()
                .w_full()
                .h(px(5.))
                .border_b_1()
                .border_color(colors.border)
                .bg(colors.element_background),
        )
}
