//! Theme browser view. Schemes, the stylix palette and provider caches load
//! off the UI thread; so does fetching a provider.

pub mod config;

use std::collections::HashSet;

use gpui::{
    AnyElement, App, ClickEvent, Context, EventEmitter, FontWeight, Pixels, Window, div,
    prelude::*, px,
};
use services::{THEME_PROVIDERS, ThemeProvider, ThemeRepository, load_stylix_scheme};
use ui::{
    ActiveTheme, Base16Palette, Color, Icon, IconName, IconSize, Radius, Spacing, TextSize, Theme,
    ThemeScheme, builtin_schemes,
};

use self::config::ThemesConfig;
use crate::config::Config;
use crate::launcher::item::Item;
use crate::launcher::view::{
    Items, LauncherView, Outcome, ViewDescriptor, ViewEvent, ViewMeta, list_row_height,
};

const THEME_ICON: IconName = IconName::Palette;

pub fn descriptor(config: &ThemesConfig) -> ViewDescriptor {
    ViewDescriptor::new(
        ViewMeta {
            id: "themes",
            prefix: config.prefix.clone().into(),
            name: "Themes",
            icon: THEME_ICON,
            description: "Browse and apply themes",
            default: false,
            in_help: true,
        },
        ThemeView::new,
    )
}

/// What the background load hands back.
struct Loaded {
    schemes: Vec<ThemeScheme>,
    stylix: Option<ThemeScheme>,
    cached: Vec<&'static str>,
}

pub struct ThemeView {
    schemes: Vec<ThemeScheme>,
    stylix: Option<ThemeScheme>,
    /// Providers with a local clone.
    cached: HashSet<&'static str>,
    /// Providers being cloned or pulled right now.
    fetching: HashSet<&'static str>,
}

impl ThemeView {
    fn new(cx: &mut Context<Self>) -> Self {
        let view = Self {
            schemes: builtin_schemes(),
            stylix: None,
            cached: HashSet::new(),
            fetching: HashSet::new(),
        };
        view.reload(cx);
        view
    }

    fn reload(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let loaded = cx.background_spawn(async { load() }).await;
            this.update(cx, |this, cx| {
                this.schemes = loaded.schemes;
                this.stylix = loaded.stylix;
                this.cached = loaded.cached.into_iter().collect();
                cx.emit(ViewEvent::ItemsChanged);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn fetch(&mut self, provider: &'static ThemeProvider, cx: &mut Context<Self>) {
        if !self.fetching.insert(provider.id) {
            return;
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { ThemeRepository::new(provider).fetch_and_cache() })
                .await;
            match result {
                Ok(schemes) => {
                    tracing::info!("Fetched {} schemes from {}", schemes.len(), provider.name)
                }
                Err(err) => tracing::error!("Failed to fetch from {}: {err}", provider.name),
            }
            this.update(cx, |this, cx| {
                this.fetching.remove(provider.id);
                this.reload(cx);
            })
            .ok();
        })
        .detach();
    }
}

fn load() -> Loaded {
    Loaded {
        schemes: build_schemes(),
        stylix: stylix_scheme(),
        cached: THEME_PROVIDERS
            .iter()
            .filter(|provider| ThemeRepository::new(provider).is_cached())
            .map(|provider| provider.id)
            .collect(),
    }
}

/// Swap only the palette, keeping the user's font size and token overrides,
/// and persist it off the UI thread.
fn apply_scheme(scheme: &ThemeScheme, cx: &mut App) {
    apply_palette(scheme.name.clone(), scheme.palette, cx);
}

/// Make `palette` the theme and save it to `theme.toml`.
pub(crate) fn apply_palette(name: gpui::SharedString, palette: Base16Palette, cx: &mut App) {
    let mut theme = cx.theme().clone();
    theme.set_palette(name, palette);
    Theme::set(theme.clone(), cx);
    cx.background_spawn(async move {
        if let Err(err) = Config::save_theme_value(&theme) {
            tracing::warn!("Failed to persist selected theme: {err}");
        }
    })
    .detach();
}

fn is_active(scheme: &ThemeScheme, theme: &Theme) -> bool {
    palette_is_active(&scheme.palette, theme)
}

pub(crate) fn palette_is_active(palette: &Base16Palette, theme: &Theme) -> bool {
    colors_match(palette.base0d, theme.colors.accent)
        && colors_match(palette.base00, theme.colors.background)
}

impl EventEmitter<ViewEvent> for ThemeView {}

impl LauncherView for ThemeView {
    fn items(&mut self, _query: &str, _cx: &mut Context<Self>) -> Items {
        Items::Ready(
            self.schemes
                .iter()
                .enumerate()
                .map(|(ix, scheme)| {
                    Item::new(ix.to_string(), scheme.name.clone())
                        .subtitle(scheme.description.clone())
                        .action("apply", "Apply")
                })
                .collect(),
        )
    }

    fn activate(&mut self, item: &Item, _action: &str, cx: &mut Context<Self>) -> Outcome {
        if let Some(scheme) = item
            .id
            .parse::<usize>()
            .ok()
            .and_then(|ix| self.schemes.get(ix))
        {
            apply_scheme(scheme, cx);
        }
        Outcome::Stay
    }

    fn row_height(&self, cx: &App) -> Pixels {
        // Card padding, border and the gap between cards.
        list_row_height(cx, &[TextSize::Small, TextSize::XSmall], px(14.))
            + Spacing::Medium.pixels() * 2.
            + px(6.)
    }

    fn render_row(
        &self,
        item: &Item,
        selected: bool,
        _window: &Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let scheme = self.schemes.get(item.id.parse::<usize>().ok()?)?;
        let theme = cx.theme();
        Some(
            div()
                .size_full()
                .px(Spacing::Medium.pixels())
                .py(px(2.))
                .child(render_theme_card(
                    scheme,
                    selected,
                    is_active(scheme, theme),
                    theme,
                ))
                .into_any_element(),
        )
    }

    fn render_header(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let mut header = div()
            .flex()
            .flex_col()
            .gap(Spacing::Medium.pixels())
            .p(Spacing::Medium.pixels());

        if let Some(stylix) = self.stylix.clone() {
            let active = is_active(&stylix, cx.theme());
            let card = render_stylix_card(&stylix, active, cx.theme());
            header = header.child(
                div()
                    .id("stylix-card")
                    .on_click(
                        cx.listener(move |_, _: &ClickEvent, _, cx| apply_scheme(&stylix, cx)),
                    )
                    .child(card),
            );
        }

        for provider in THEME_PROVIDERS {
            let card = render_provider_card(
                provider,
                self.cached.contains(provider.id),
                self.fetching.contains(provider.id),
                cx.theme(),
            );
            header = header.child(
                div()
                    .id(provider.id)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.fetch(provider, cx);
                    }))
                    .child(card),
            );
        }

        Some(header.into_any_element())
    }
}

fn stylix_scheme() -> Option<ThemeScheme> {
    let b16 = load_stylix_scheme()?;
    let p = &b16.palette;
    let palette = Base16Palette::from_hex(&[
        &p.base00, &p.base01, &p.base02, &p.base03, &p.base04, &p.base05, &p.base06, &p.base07,
        &p.base08, &p.base09, &p.base0a, &p.base0b, &p.base0c, &p.base0d, &p.base0e, &p.base0f,
    ])
    .ok()?;

    Some(ThemeScheme::new(
        b16.name,
        format!("Stylix — {}", b16.author),
        palette,
    ))
}

fn build_schemes() -> Vec<ThemeScheme> {
    let mut schemes = builtin_schemes();

    for provider in THEME_PROVIDERS {
        let repo = ThemeRepository::new(provider);
        for b16 in repo.load_cached() {
            let p = &b16.palette;
            let palette = match Base16Palette::from_hex(&[
                &p.base00, &p.base01, &p.base02, &p.base03, &p.base04, &p.base05, &p.base06,
                &p.base07, &p.base08, &p.base09, &p.base0a, &p.base0b, &p.base0c, &p.base0d,
                &p.base0e, &p.base0f,
            ]) {
                Ok(c) => c,
                Err(_) => continue,
            };

            schemes.push(ThemeScheme::new(
                b16.name,
                format!("{} — {}", provider.name, b16.author),
                palette,
            ));
        }
    }

    schemes
}

fn colors_match(a: gpui::Hsla, b: gpui::Hsla) -> bool {
    (a.h - b.h).abs() < 0.01 && (a.s - b.s).abs() < 0.01 && (a.l - b.l).abs() < 0.01
}

fn render_stylix_card(scheme: &ThemeScheme, is_active: bool, theme: &Theme) -> AnyElement {
    let preview_colors = scheme.preview_colors();

    div()
        .w_full()
        .p(Spacing::Large.pixels())
        .rounded(Radius::Large.pixels())
        .bg(theme.colors.surface_background)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(Spacing::Medium.pixels())
                .child(
                    Icon::new(THEME_ICON)
                        .size(IconSize::Large)
                        .color(Color::Accent),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .text_size(TextSize::Medium.rems())
                                .text_color(theme.colors.text)
                                .font_weight(FontWeight::MEDIUM)
                                .child(scheme.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(TextSize::XSmall.rems())
                                .text_color(theme.colors.text_disabled)
                                .child(scheme.description.clone()),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(Spacing::Medium.pixels())
                .child(render_color_strip(&preview_colors))
                .when(is_active, |el| el.child(active_badge(theme))),
        )
        .into_any_element()
}

fn render_provider_card(
    provider: &'static ThemeProvider,
    is_downloaded: bool,
    is_fetching: bool,
    theme: &Theme,
) -> AnyElement {
    let interactive_hover = theme.colors.element_hover;
    let (icon, action) = if is_fetching {
        (IconName::Refresh, format!("Fetching {}…", provider.name))
    } else if is_downloaded {
        (IconName::Refresh, format!("Update {}", provider.name))
    } else {
        (IconName::Download, format!("Download {}", provider.name))
    };

    div()
        .w_full()
        .px(Spacing::Large.pixels())
        .py(Spacing::Medium.pixels())
        .rounded(Radius::Large.pixels())
        .bg(theme.colors.surface_background)
        .when(!is_fetching, |el| {
            el.cursor_pointer().hover(move |s| s.bg(interactive_hover))
        })
        .flex()
        .items_center()
        .gap(Spacing::Medium.pixels())
        .child(Icon::new(icon).size(IconSize::Large).color(if is_fetching {
            Color::Muted
        } else {
            Color::Accent
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(TextSize::Small.rems())
                        .text_color(theme.colors.text)
                        .font_weight(FontWeight::MEDIUM)
                        .child(action),
                )
                .child(
                    div()
                        .text_size(TextSize::XSmall.rems())
                        .text_color(theme.colors.text_disabled)
                        .child(provider.repo),
                ),
        )
        .into_any_element()
}

fn render_theme_card(
    scheme: &ThemeScheme,
    is_selected: bool,
    is_active: bool,
    theme: &Theme,
) -> AnyElement {
    let accent_selection = theme.colors.element_selected;
    let interactive_hover = theme.colors.element_hover;
    let border_default = theme.colors.border;
    let accent_primary = theme.colors.accent;
    let preview_colors = scheme.preview_colors();

    div()
        .size_full()
        .px(Spacing::Large.pixels())
        .rounded(Radius::Large.pixels())
        .border_1()
        .cursor_pointer()
        .when(is_selected, move |el| {
            el.bg(accent_selection).border_color(accent_primary)
        })
        .when(!is_selected, move |el| {
            el.border_color(border_default)
                .hover(move |s| s.bg(interactive_hover))
        })
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(TextSize::Small.rems())
                        .text_color(theme.colors.text)
                        .font_weight(FontWeight::MEDIUM)
                        .child(scheme.name.clone()),
                )
                .child(
                    div()
                        .text_size(TextSize::XSmall.rems())
                        .text_color(theme.colors.text_disabled)
                        .child(scheme.description.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(Spacing::Medium.pixels())
                .child(render_color_strip(&preview_colors))
                .when(is_active, |el| el.child(active_badge(theme))),
        )
        .into_any_element()
}

fn active_badge(theme: &Theme) -> impl IntoElement {
    div()
        .px(Spacing::Medium.pixels())
        .py(px(2.))
        .rounded(Radius::Small.pixels())
        .bg(theme.colors.accent)
        .text_size(TextSize::XSmall.rems())
        .text_color(theme.colors.background)
        .font_weight(FontWeight::BOLD)
        .child("Active")
}

fn render_color_strip(colors: &[gpui::Hsla]) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(2.))
        .children(
            colors
                .iter()
                .map(|&color| div().w(px(14.)).h(px(14.)).rounded(px(3.)).bg(color)),
        )
        .into_any_element()
}
