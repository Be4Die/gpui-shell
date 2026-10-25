//! One full-output layer per display, under the panels: draws the screen frame
//! for `bar.style = "frame"`, and while a panel is open catches clicks outside
//! it to close it. Its input region is empty otherwise, so it never gets in
//! the way.
//!
//! The frame keeps windows off its edges through [`FrameExclusion`] strips,
//! because the full-output layer itself must ignore exclusive zones.

use futures::StreamExt;
use futures_signals::map_ref;
use futures_signals::signal::SignalExt;
use gpui::{
    App, Bounds, Context, DisplayId, Pixels, Size, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, layer_shell::*, point, prelude::*, px,
};
use ui::ActiveTheme;
use ui::patterns::{ScreenFrame, frame::outside_bar};

use crate::config::{ActiveConfig, BarConfig, BarPosition, BarStyle, ConfigSignals};
use crate::state::{display_extent, record_window_display, watch};
use crate::windows::{PanelsConfig, ShellWindows};

/// What forces the frame windows to be reopened.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    /// The full-output layer has a job: drawing, or catching outside clicks.
    enabled: bool,
    position: BarPosition,
    /// Thickness of the exclusion strips; `None` when there are none.
    strips: Option<f32>,
}

impl Layout {
    fn new(bar: &BarConfig, panels: &PanelsConfig) -> Self {
        let framed = bar.style == BarStyle::Frame;
        Self {
            enabled: framed || panels.close_on_click_outside,
            position: bar.position,
            strips: (framed && bar.frame.thickness > 0.).then_some(bar.frame.thickness),
        }
    }
}

struct Frame {
    panel_open: bool,
    input_region: Option<Vec<Bounds<Pixels>>>,
}

impl Frame {
    fn new(cx: &mut Context<Self>) -> Self {
        let panel_open = ShellWindows::panel_open_signal(cx);
        watch(cx, panel_open, |this, open, cx| {
            this.panel_open = open;
            cx.notify();
        });
        let bar = ConfigSignals::global(cx).bar.signal_cloned();
        watch(cx, bar, |_, _, cx| cx.notify());

        Self {
            panel_open: false,
            input_region: None,
        }
    }
}

impl Render for Frame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = cx.config();
        let bar = &config.bar;
        let edge = bar.position.edge();
        let viewport = Bounds {
            origin: point(px(0.), px(0.)),
            size: window.viewport_size(),
        };

        // The bar keeps its own clicks, so a click on another widget swaps panels.
        let region = if self.panel_open && config.panels.close_on_click_outside {
            vec![outside_bar(viewport, edge, px(bar.depth()))]
        } else {
            Vec::new()
        };
        if self.input_region.as_ref() != Some(&region) {
            window.set_input_region(Some(&region));
            self.input_region = Some(region);
        }

        let frame = (bar.style == BarStyle::Frame).then(|| {
            let theme = cx.theme();
            ScreenFrame::new(edge, px(bar.size), bar.background_color(theme))
                .thickness(px(bar.frame.thickness.max(0.)))
                .radius(px(bar.frame.radius.max(0.)))
                .border(bar.show_border.then_some(theme.colors().border))
        });

        div()
            .id("frame")
            .size_full()
            .on_any_mouse_down(|_, _, cx| ShellWindows::close_panel(cx))
            .children(frame)
    }
}

/// An invisible strip that reserves the frame's thickness on one edge.
struct FrameExclusion {
    input_cleared: bool,
}

impl Render for FrameExclusion {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if !self.input_cleared {
            window.set_input_region(Some(&[]));
            self.input_cleared = true;
        }
        div()
    }
}

fn layer_window(
    display_id: Option<DisplayId>,
    size: Size<Pixels>,
    app_id: &str,
    options: LayerShellOptions,
) -> WindowOptions {
    WindowOptions {
        display_id,
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.), px(0.)),
            size,
        })),
        app_id: Some(app_id.to_string()),
        window_background: WindowBackgroundAppearance::Transparent,
        kind: WindowKind::LayerShell(LayerShellOptions {
            keyboard_interactivity: KeyboardInteractivity::None,
            ..options
        }),
        focus: false,
        ..Default::default()
    }
}

fn open_frame(display_id: Option<DisplayId>, cx: &mut App) -> bool {
    let options = layer_window(
        display_id,
        display_extent(display_id, cx),
        "gpuishell-frame",
        LayerShellOptions {
            namespace: "frame".to_string(),
            layer: Layer::Top,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            // Span the whole output, bar included.
            exclusive_zone: Some(px(-1.)),
            ..Default::default()
        },
    );
    match cx.open_window(options, move |window, cx| {
        record_window_display(window.window_handle(), display_id, cx);
        cx.new(Frame::new)
    }) {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!("Failed to open frame window: {err}");
            false
        }
    }
}

fn open_strip(
    display_id: Option<DisplayId>,
    edge: BarPosition,
    thickness: f32,
    cx: &mut App,
) -> bool {
    let extent = display_extent(display_id, cx);
    let (size, anchor) = match edge {
        BarPosition::Left => (
            Size::new(px(thickness), extent.height),
            Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
        ),
        BarPosition::Right => (
            Size::new(px(thickness), extent.height),
            Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
        ),
        BarPosition::Top => (
            Size::new(extent.width, px(thickness)),
            Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        ),
        BarPosition::Bottom => (
            Size::new(extent.width, px(thickness)),
            Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        ),
    };
    let options = layer_window(
        display_id,
        size,
        "gpuishell-frame-exclusion",
        LayerShellOptions {
            namespace: "frame-exclusion".to_string(),
            // Hyprland lays out exclusive zones by layer, niri by creation
            // order: Overlay plus `reopen` puts the strips after the bar either way.
            layer: Layer::Overlay,
            anchor,
            exclusive_zone: Some(px(thickness)),
            ..Default::default()
        },
    );
    match cx.open_window(options, |_, cx| {
        cx.new(|_| FrameExclusion {
            input_cleared: false,
        })
    }) {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!("Failed to open frame exclusion strip: {err}");
            false
        }
    }
}

fn open_all(layout: Layout, cx: &mut App) {
    if !layout.enabled {
        return;
    }
    let displays: Vec<_> = cx
        .displays()
        .iter()
        .map(|display| Some(display.id()))
        .collect();
    let displays = if displays.is_empty() {
        vec![None]
    } else {
        displays
    };

    for display_id in displays {
        open_frame(display_id, cx);
        if let Some(thickness) = layout.strips {
            for edge in [
                BarPosition::Top,
                BarPosition::Right,
                BarPosition::Bottom,
                BarPosition::Left,
            ] {
                if edge != layout.position {
                    open_strip(display_id, edge, thickness, cx);
                }
            }
        }
    }
}

fn reload(layout: Layout, cx: &mut App) {
    let old_windows: Vec<_> = cx
        .windows()
        .into_iter()
        .filter(|handle| {
            handle.downcast::<Frame>().is_some() || handle.downcast::<FrameExclusion>().is_some()
        })
        .collect();
    for handle in old_windows {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    open_all(layout, cx);
}

/// Reopen the frame windows after the bar's. niri hands out exclusive
/// zones in creation order, so strips opened first would shorten the bar.
pub fn reopen(cx: &mut App) {
    let config = cx.config();
    reload(Layout::new(&config.bar, &config.panels), cx);
}

pub fn init(cx: &mut App) {
    let signals = ConfigSignals::global(cx);
    let layout = map_ref! {
        let bar = signals.bar.signal_cloned(),
        let panels = signals.panels.signal_cloned() => Layout::new(bar, panels)
    }
    .dedupe()
    .to_stream();

    // The first open comes from the bar, see [`reopen`].
    cx.spawn(async move |cx| {
        let mut layout = Box::pin(layout.skip(1));
        while let Some(layout) = layout.next().await {
            cx.update(|cx| reload(layout, cx));
        }
    })
    .detach();
}
