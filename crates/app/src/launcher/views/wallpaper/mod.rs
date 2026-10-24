//! Wallpaper view: a grid of thumbnails. Applying one can also generate a
//! matching theme with matugen.

pub mod config;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui::{
    AnyElement, App, Context, EntityId, EventEmitter, ObjectFit, Pixels, SharedString, Window, div,
    img, prelude::*, px, size,
};
use services::WallpaperCommand;
use ui::{
    ActiveTheme, Base16Palette, Color, Icon, IconName, IconSize, Label, LabelCommon, Radius,
    Spacing, Switch, SwitchSize, TextSize, Theme, ToggleState,
};

use self::config::WallpaperConfig;
use crate::config::State;
use crate::launcher::item::{Item, ItemIcon};
use crate::launcher::row::thumbnail_image;
use crate::launcher::view::{
    Body, Items, LauncherView, Outcome, ViewDescriptor, ViewEvent, ViewMeta, list_row_height,
};
use crate::state::AppState;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp"];

/// Thumbnail height in a card; the width follows the column.
const THUMB_HEIGHT: Pixels = px(100.);
/// Widest a card gets in the default window, for sizing the thumbnail.
const THUMB_WIDTH: Pixels = px(190.);

pub fn descriptor(config: &WallpaperConfig) -> ViewDescriptor {
    let config = config.clone();
    ViewDescriptor::new(
        ViewMeta {
            id: "wallpaper",
            prefix: config.prefix.clone().into(),
            name: "Wallpaper",
            icon: IconName::Image,
            description: "Browse and set wallpapers",
            default: false,
            in_help: true,
        },
        move |cx| WallpaperView::new(&config, cx),
    )
}

struct WallpaperEntry {
    path: PathBuf,
    name: String,
    extension: String,
    /// Part of the thumbnail key, so a file overwritten in place re-renders.
    mtime: Option<SystemTime>,
}

pub struct WallpaperView {
    directory: PathBuf,
    entries: Vec<WallpaperEntry>,
    scanning: bool,
    columns: usize,
    matugen: bool,
    dark_mode: bool,
    matugen_scheme: String,
    /// Thumbnail loads notify this view, which redraws the launcher.
    owner: EntityId,
}

impl WallpaperView {
    fn new(config: &WallpaperConfig, cx: &mut Context<Self>) -> Self {
        let directory = expand_tilde(&config.directory);
        let scan_dir = directory.clone();
        cx.spawn(async move |this, cx| {
            let entries = cx
                .background_spawn(async move { scan_wallpapers(&scan_dir) })
                .await;
            this.update(cx, |this, cx| {
                this.entries = entries;
                this.scanning = false;
                cx.emit(ViewEvent::ItemsChanged);
            })
            .ok();
        })
        .detach();

        let (matugen, dark_mode) = State::wallpaper_toggles(cx);
        Self {
            directory,
            entries: Vec::new(),
            scanning: true,
            columns: config.columns.max(1),
            matugen,
            dark_mode,
            matugen_scheme: config.matugen_scheme.clone(),
            owner: cx.entity_id(),
        }
    }

    fn set_toggles(&mut self, matugen: bool, dark_mode: bool, cx: &mut Context<Self>) {
        self.matugen = matugen;
        self.dark_mode = dark_mode;
        State::set_wallpaper_toggles(matugen, dark_mode, cx);
        cx.notify();
    }

    fn apply(&self, path: PathBuf, cx: &mut Context<Self>) {
        AppState::wallpaper(cx).dispatch(WallpaperCommand::SetWallpaper(path.clone()));
        if !self.matugen {
            return;
        }

        let mode = if self.dark_mode { "dark" } else { "light" };
        let scheme = self.matugen_scheme.clone();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_spawn({
                    let path = path.clone();
                    async move { Base16Palette::generate_from_wallpaper(&path, mode, &scheme, 0) }
                })
                .await;
            match result {
                Ok(palette) => {
                    tracing::debug!("Matugen palette generated for {}", path.display());
                    // Keep the user's font size and token overrides.
                    cx.update(|cx| {
                        let mut theme = cx.theme().clone();
                        theme.set_palette("Wallpaper", palette);
                        Theme::set(theme, cx);
                    });
                }
                Err(err) => tracing::warn!("Failed to generate matugen theme: {err}"),
            }
        })
        .detach();
    }
}

impl EventEmitter<ViewEvent> for WallpaperView {}

impl LauncherView for WallpaperView {
    fn items(&mut self, _query: &str, _cx: &mut Context<Self>) -> Items {
        Items::Ready(
            self.entries
                .iter()
                .map(|entry| {
                    Item::new(entry.path.to_string_lossy().to_string(), entry.name.clone())
                        .subtitle(entry.extension.clone())
                        .icon(ItemIcon::Thumbnail {
                            path: entry.path.clone(),
                            mtime: entry.mtime,
                        })
                        .action("apply", "Apply")
                })
                .collect(),
        )
    }

    fn activate(&mut self, item: &Item, _action: &str, cx: &mut Context<Self>) -> Outcome {
        self.apply(PathBuf::from(item.id.as_ref()), cx);
        Outcome::Stay
    }

    fn body(&mut self, _cx: &mut Context<Self>) -> Body {
        Body::Grid {
            columns: self.columns,
        }
    }

    fn row_height(&self, cx: &App) -> Pixels {
        THUMB_HEIGHT + list_row_height(cx, &[TextSize::Small], px(0.)) + Spacing::Medium.pixels()
    }

    fn render_row(
        &self,
        item: &Item,
        selected: bool,
        window: &Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let ItemIcon::Thumbnail { path, mtime } = &item.icon else {
            return None;
        };
        let colors = cx.theme().colors();
        let image = thumbnail_image(
            path,
            *mtime,
            size(THUMB_WIDTH, THUMB_HEIGHT),
            self.owner,
            window,
            cx,
        );

        Some(
            div()
                .size_full()
                .p(Spacing::XSmall.pixels())
                .rounded(Radius::Medium.pixels())
                .border_1()
                .border_color(if selected {
                    colors.accent
                } else {
                    gpui::transparent_black()
                })
                .when(selected, |el| el.bg(colors.element_selected))
                .when(!selected, |el| el.hover(|s| s.bg(colors.element_hover)))
                .cursor_pointer()
                .flex()
                .flex_col()
                .gap(Spacing::XSmall.pixels())
                .child(
                    div()
                        .w_full()
                        .h(THUMB_HEIGHT)
                        .rounded(Radius::Small.pixels())
                        .overflow_hidden()
                        .bg(colors.element_background)
                        .children(
                            image.map(|image| img(image).size_full().object_fit(ObjectFit::Cover)),
                        ),
                )
                .child(
                    Label::new(item.title.clone())
                        .size(TextSize::Small)
                        .truncate(),
                )
                .into_any_element(),
        )
    }

    fn render_header(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let colors = cx.theme().colors();
        let (matugen, dark_mode) = (self.matugen, self.dark_mode);
        let mode_icon = |icon: IconName, on: bool| {
            Icon::new(icon).size(IconSize::XSmall).color(if on {
                Color::Accent
            } else {
                Color::Muted
            })
        };

        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(Spacing::XLarge.pixels())
                .py(Spacing::Medium.pixels())
                .bg(colors.surface_background)
                .border_b_1()
                .border_color(colors.border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Spacing::Large.pixels())
                        .child(
                            Icon::new(IconName::Palette)
                                .size(IconSize::Small)
                                .color(if matugen { Color::Accent } else { Color::Muted }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(Label::new("Material You theming").size(TextSize::Small))
                                .child(
                                    Label::new("Generate theme colors from the wallpaper")
                                        .size(TextSize::XSmall)
                                        .color(Color::Muted),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Spacing::Medium.pixels())
                        .when(matugen, |el| {
                            el.child(mode_icon(IconName::Sun, !dark_mode))
                                .child(
                                    Switch::new("matugen-dark-mode", dark_mode)
                                        .size(SwitchSize::Small)
                                        .on_click(cx.listener(
                                            move |this, state: &ToggleState, _, cx| {
                                                this.set_toggles(matugen, state.selected(), cx);
                                            },
                                        )),
                                )
                                .child(mode_icon(IconName::Moon, dark_mode))
                        })
                        .child(
                            Switch::new("matugen-toggle", matugen)
                                .size(SwitchSize::Medium)
                                .on_click(cx.listener(move |this, state: &ToggleState, _, cx| {
                                    this.set_toggles(state.selected(), dark_mode, cx);
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    fn empty_text(&self) -> Option<SharedString> {
        let dir = self.directory.display();
        Some(if self.scanning {
            format!("Scanning {dir}…").into()
        } else {
            format!("No images in {dir}").into()
        })
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

fn scan_wallpapers(dir: &Path) -> Vec<WallpaperEntry> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        tracing::warn!("Could not read wallpaper directory: {}", dir.display());
        return Vec::new();
    };

    let mut entries: Vec<WallpaperEntry> = read_dir
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_str()?.to_lowercase();
            if !path.is_file() || !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            Some(WallpaperEntry {
                path,
                name,
                extension: extension.to_uppercase(),
                mtime,
            })
        })
        .collect();

    entries.sort_by_cached_key(|entry| entry.name.to_lowercase());
    entries
}
