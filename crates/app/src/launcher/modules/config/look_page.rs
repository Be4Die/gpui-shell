//! Pick the bar's look: one row per preset with a miniature of it, Enter
//! applies the `[bar]` keys and the theme's radius scale together. The
//! config edits stay pending until Save, like every other editor page; the
//! theme change is written to `theme.toml` at once, like a scheme switch.

use gpui::{AnyElement, App, Context, Hsla, Window, div, prelude::*, px};
use toml_edit::Value;
use ui::patterns::{BarChip, BarEdge, BarRhythm, BarSegment, BarShape, BarSurface, ChipGeometry};
use ui::{
    ActiveTheme, Color, Icon, IconName, IconSize, Label, LabelCommon, Radius, Spacing, TextSize,
    h_flex,
};

use super::schema::{ConfigPath, to_value};
use super::section_page::{file_actions, unsaved_banner, watch_revision};
use crate::bar::config::BarConfig;
use crate::bar::look::Look;
use crate::launcher::action::{Action, ActionPanel, Outcome};
use crate::launcher::content::{Accessory, Content, Detail, Item, List, Section};
use crate::launcher::modules::theme::apply_theme;
use crate::launcher::page::{LauncherPage, Page};

pub struct LookPage {
    path: ConfigPath<BarConfig>,
}

impl LookPage {
    pub fn open(path: ConfigPath<BarConfig>, cx: &mut App) -> Page {
        Page::new(cx.new(|cx| {
            watch_revision(cx);
            Self { path }
        }))
    }
}

/// Apply `look`: every `[bar]` key it owns as its own pending edit (unset
/// ones removed, so defaults apply again), then the theme's radius scale.
pub fn apply_look(look: Look, path: &ConfigPath<BarConfig>, cx: &mut App) {
    let mut bar = path.read(cx).clone();
    look.apply(&mut bar);

    let set: [(&str, Value); 5] = [
        ("style", to_value(&bar.style)),
        ("show_border", to_value(&bar.show_border)),
        ("widget_background", to_value(&bar.widget_background)),
        ("widget_border", to_value(&bar.widget_border)),
        ("widget_stretch", to_value(&bar.widget_stretch)),
    ];
    for (key, value) in set {
        path.set_raw(path.keys_for(key), value, bar.clone(), cx);
    }
    match bar.padding {
        Some(padding) => path.set_raw(path.keys_for("padding"), to_value(&padding), bar.clone(), cx),
        None => path.remove_raw(path.keys_for("padding"), bar.clone(), cx),
    }
    for key in ["segments.gap", "segments.inset", "segments.padding"] {
        path.remove_raw(path.keys_for(key), bar.clone(), cx);
    }

    let mut theme = cx.theme().clone();
    theme.set_radius_scale(look.radius_scale());
    apply_theme(theme, cx);
}

impl LauncherPage for LookPage {
    fn content(&mut self, _query: &str, cx: &mut Context<Self>) -> Content {
        let bar = self.path.read(cx);
        let current = Look::current(bar, cx.theme().radius_scale);
        let items = Look::ALL.into_iter().map(|look| {
            let path = self.path.clone();
            let mut item = Item::new(look.label().to_lowercase(), look.label())
                .subtitle(look.description())
                .icon(match look {
                    Look::Blocky => IconName::Square,
                    Look::Rounded => IconName::Circle,
                    Look::Floating => IconName::Layout,
                })
                .keywords("look preset blocky rounded square pill floating radius")
                .detail(Detail::new().widget(move |cx| preview(look, cx)))
                .actions(file_actions(ActionPanel::new().action(
                    Action::new("Apply", move |_, cx| {
                        apply_look(look, &path, cx);
                        Outcome::Stay
                    })
                    .icon(IconName::Check),
                )));
            if current == Some(look) {
                item = item.accessory(Accessory::tag("current"));
            }
            item
        });
        List::new()
            .title("Bar look")
            .placeholder("Pick a look, then Save")
            .actions(file_actions(ActionPanel::new()))
            .section(
                Section::new("Presets")
                    .subtitle(
                        "Sets style, borders, widget chrome, stretch and end padding under [bar], \
                         and radius_scale in theme.toml. Widgets and their order are kept; group \
                         widgets in Layout to share a pill.",
                    )
                    .items(items),
            )
            .into()
    }

    fn render_header(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        unsaved_banner(cx)
    }
}

/// The bar as `look` would draw it, at the preset's own radius scale so
/// the picture does not depend on the theme the user is leaving.
fn preview(look: Look, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let scale = look.radius_scale();
    let radius = |token: Radius| px(token.scaled(scale));
    let bar = px(32.);
    let rhythm = BarRhythm::derive(bar);
    let (chip_bg, chip_border): (Hsla, Option<Hsla>) = match look {
        Look::Floating => (colors.surface_background, Some(colors.border)),
        Look::Blocky | Look::Rounded => (gpui::transparent_black(), None),
    };
    let chip_geometry = |grouped: bool| match look {
        Look::Blocky => ChipGeometry::stretched(&rhythm, grouped, radius(Radius::Large)),
        Look::Rounded | Look::Floating => ChipGeometry::derive(&rhythm, grouped, radius(Radius::Large)),
    };
    let chip = |content: AnyElement, grouped: bool| {
        BarChip::new(content)
            .geometry(chip_geometry(grouped))
            .background(chip_bg)
            .border(chip_border)
            .into_any_element()
    };
    let stat = |icon: IconName, text: &'static str| {
        h_flex()
            .gap(rhythm.breath)
            .child(Icon::new(icon).size(IconSize::Small))
            .child(Label::new(text).size(TextSize::Small))
            .into_any_element()
    };
    let cell_side = rhythm.chip() - rhythm.unit * 2.;
    let cell = |label: &'static str, active: bool| {
        let side = if look == Look::Blocky { bar } else { cell_side };
        div()
            .flex()
            .items_center()
            .justify_center()
            .h(side)
            .min_w(if active && look != Look::Blocky {
                px((f32::from(side) * 1.4).round())
            } else {
                side
            })
            .px(rhythm.unit)
            .rounded(if look == Look::Blocky {
                radius(Radius::Large)
            } else {
                px(Radius::Large.scaled(scale)) - rhythm.unit
            })
            .bg(if active {
                colors.accent
            } else {
                colors.elevated_surface_background
            })
            .child(
                Label::new(label)
                    .size(TextSize::Small)
                    .color(Color::Custom(if active {
                        colors.background
                    } else {
                        colors.text
                    })),
            )
    };
    let cells = chip(
        h_flex()
            .gap(if look == Look::Blocky {
                px(0.)
            } else {
                rhythm.breath
            })
            .child(cell("1", false))
            .child(cell("2", true))
            .child(cell("3", false))
            .into_any_element(),
        true,
    );

    // Bubbles put the fill on the segments; the other looks on the bar.
    let (bar_fill, segment_fill, inset, gap, segment_padding) = match look {
        Look::Rounded => (
            gpui::transparent_black(),
            Some(colors.background),
            px(3.),
            px(6.),
            px(4.),
        ),
        Look::Blocky | Look::Floating => (colors.background, None, px(0.), px(0.), px(0.)),
    };
    let segment = |children: Vec<AnyElement>| {
        BarSegment::new(false)
            .fill(segment_fill)
            .border(if look == Look::Rounded {
                Some(colors.border)
            } else {
                None
            })
            .radius(px(Radius::Full.scaled(scale)).min((bar - inset * 2.) / 2.))
            .thickness(Some(bar - inset * 2.))
            .padding(segment_padding)
            .children(children)
            .into_any_element()
    };
    let shape = match look {
        Look::Floating => BarShape::Floating {
            radius: radius(Radius::XLarge),
        },
        Look::Blocky | Look::Rounded => BarShape::Attached,
    };

    div()
        .w_full()
        .p(Spacing::Medium.pixels())
        .rounded(Radius::Medium.pixels())
        .bg(colors.surface_background)
        .child(
            div().w_full().h(bar).child(
                BarSurface::new(BarEdge::Top)
                    .shape(shape)
                    .border(look == Look::Floating)
                    .background(bar_fill)
                    .padding(if look == Look::Rounded { gap } else { rhythm.unit })
                    .inset(inset)
                    .segment_gap(gap)
                    .start([
                        segment(vec![chip(
                            Icon::new(IconName::Layers)
                                .size(IconSize::Small)
                                .into_any_element(),
                            false,
                        )]),
                        segment(vec![cells]),
                        segment(vec![chip(stat(IconName::Cpu, "12%"), false)]),
                    ])
                    .end([
                        segment(vec![chip(stat(IconName::Wifi, "Home"), false)]),
                        segment(vec![chip(stat(IconName::Clock, "09:41"), false)]),
                    ]),
            ),
        )
        .into_any_element()
}
