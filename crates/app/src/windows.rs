//! The shell's transient windows.
//!
//! Panels - tray menus, the control center, the notification center, the dock
//! menus - are one-at-a-time layer-shell windows anchored next to whatever
//! opened them. The launcher and the notification popups are windows of their
//! own. [`ShellWindows`] is the single registry of what is open, so the rest
//! of the shell asks it instead of guessing: the dock, for one, must not read
//! "the compositor reports no focused toplevel" as "the desktop is bare" while
//! it is our own panel that took the keyboard.
//!
//! A layer-shell window cannot be sized from its content by the compositor,
//! so a panel that wants to hug its content opens at a guess, measures what it
//! renders, and asks for that size. [`PanelHost`] does this for every panel
//! sized [`PanelSize::FitHeight`] and paints nothing until the window has the
//! size it asked for - the blank first frame is transparent, where a frame of
//! content at the wrong size is the flicker.

use gpui::{
    AnyElement, AnyWindowHandle, App, AvailableSpace, Bounds, Context, DisplayId, Entity, Global,
    Pixels, Point, Render, Size, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, div, layer_shell::*, point, prelude::*, px, size,
};
use ui::ActiveTheme;

use crate::config::BarPosition;

// ============================================================================
// Panel sizing and placement
// ============================================================================

/// How a panel window is sized.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanelSize {
    /// A window of exactly this size; the content lays out inside it.
    Fixed { width: f32, height: f32 },
    /// A window this wide whose height follows the content, up to
    /// `max_height` and never past the output's usable area. Content taller
    /// than the cap has to scroll itself.
    FitHeight { width: f32, max_height: f32 },
}

impl PanelSize {
    /// The size the window opens at. A fitted panel opens at its cap and
    /// shrinks to its content before it paints, so placement can assume the
    /// largest it will ever be.
    pub fn initial(&self) -> Size<Pixels> {
        match self {
            PanelSize::Fixed { width, height } => size(px(*width), px(*height)),
            PanelSize::FitHeight { width, max_height } => size(px(*width), px(*max_height)),
        }
    }
}

/// Panel configuration for positioning and sizing.
#[derive(Clone, Debug)]
pub struct PanelConfig {
    pub size: PanelSize,
    pub anchor: Anchor,
    pub margin: (f32, f32, f32, f32), // top, right, bottom, left
    pub namespace: String,
}

impl Default for PanelConfig {
    fn default() -> Self {
        PanelConfig {
            size: PanelSize::FitHeight {
                width: 320.0,
                max_height: 400.0,
            },
            anchor: Anchor::TOP | Anchor::RIGHT,
            margin: (5.0, 5.0, 5.0, 5.0),
            namespace: "panel".to_string(),
        }
    }
}

/// Resolve panel anchor/margin from a click position, in window coordinates.
pub fn panel_placement_from_event(
    bar_position: BarPosition,
    position: Point<Pixels>,
    window: &Window,
    cx: &App,
    panel_size: Size<Pixels>,
) -> (Anchor, (f32, f32, f32, f32)) {
    let (display_bounds, usable_bounds) = window
        .display(cx)
        .map(|display| (display.bounds(), display.visible_bounds()))
        .unwrap_or_else(|| {
            let bounds = window.bounds();
            (bounds, bounds)
        });
    let click = point(
        window.bounds().origin.x + position.x,
        window.bounds().origin.y + position.y,
    );
    panel_placement_from_click(
        bar_position,
        click,
        panel_size,
        display_bounds,
        usable_bounds,
    )
}

/// Resolve panel anchor/margin from a click position.
pub fn panel_placement_from_click(
    bar_position: BarPosition,
    click: Point<Pixels>,
    panel_size: Size<Pixels>,
    display_bounds: Bounds<Pixels>,
    usable_bounds: Bounds<Pixels>,
) -> (Anchor, (f32, f32, f32, f32)) {
    let anchor_horizontal = match bar_position {
        BarPosition::Left => Anchor::LEFT,
        BarPosition::Right => Anchor::RIGHT,
        BarPosition::Top | BarPosition::Bottom => {
            if click.x < display_bounds.center().x {
                Anchor::LEFT
            } else {
                Anchor::RIGHT
            }
        }
    };

    let anchor_vertical = match bar_position {
        BarPosition::Top => Anchor::TOP,
        BarPosition::Bottom => Anchor::BOTTOM,
        BarPosition::Left | BarPosition::Right => {
            if click.y < display_bounds.center().y {
                Anchor::TOP
            } else {
                Anchor::BOTTOM
            }
        }
    };

    let anchor = anchor_horizontal | anchor_vertical;
    let margin = margin_from_click(
        bar_position,
        anchor,
        click,
        panel_size,
        display_bounds,
        usable_bounds,
    );

    (anchor, margin)
}

fn margin_from_click(
    bar_position: BarPosition,
    anchor: Anchor,
    click: Point<Pixels>,
    panel_size: Size<Pixels>,
    display_bounds: Bounds<Pixels>,
    usable_bounds: Bounds<Pixels>,
) -> (f32, f32, f32, f32) {
    let display_origin = display_bounds.origin;
    let display_size = display_bounds.size;
    let usable_origin = usable_bounds.origin;
    let usable_size = usable_bounds.size;

    let click_x: f32 = (click.x - display_origin.x).into();
    let click_y: f32 = (click.y - display_origin.y).into();
    let panel_w: f32 = panel_size.width.into();
    let panel_h: f32 = panel_size.height.into();
    let disp_w: f32 = display_size.width.into();
    let disp_h: f32 = display_size.height.into();

    let avail_x = (disp_w - panel_w).max(0.0);
    let avail_y = (disp_h - panel_h).max(0.0);

    let inset_left: f32 = (usable_origin.x - display_origin.x).into();
    let inset_top: f32 = (usable_origin.y - display_origin.y).into();
    let inset_right: f32 =
        (display_origin.x + display_size.width - (usable_origin.x + usable_size.width)).into();
    let inset_bottom: f32 =
        (display_origin.y + display_size.height - (usable_origin.y + usable_size.height)).into();

    let mut min_x = inset_left;
    let mut max_x = disp_w - inset_right - panel_w;
    if max_x < min_x {
        min_x = 0.0;
        max_x = avail_x;
    }

    let mut min_y = inset_top;
    let mut max_y = disp_h - inset_bottom - panel_h;
    if max_y < min_y {
        min_y = 0.0;
        max_y = avail_y;
    }

    let mut origin_x = (click_x - (panel_w / 2.0)).clamp(min_x, max_x);
    let mut origin_y = (click_y - (panel_h / 2.0)).clamp(min_y, max_y);

    match bar_position {
        BarPosition::Top => {
            origin_y = min_y;
        }
        BarPosition::Bottom => {
            origin_y = max_y;
        }
        BarPosition::Left => {
            origin_x = min_x;
        }
        BarPosition::Right => {
            origin_x = max_x;
        }
    }

    let mut top = 0.0;
    let mut right = 0.0;
    let mut bottom = 0.0;
    let mut left = 0.0;

    if anchor.contains(Anchor::TOP) {
        top = origin_y;
    }
    if anchor.contains(Anchor::BOTTOM) {
        bottom = disp_h - (origin_y + panel_h);
    }
    if anchor.contains(Anchor::LEFT) {
        left = origin_x;
    }
    if anchor.contains(Anchor::RIGHT) {
        right = disp_w - (origin_x + panel_w);
    }

    (top, right, bottom, left)
}

// ============================================================================
// Fitting a window to its content
// ============================================================================

/// The resize handshake of a window that follows its content.
///
/// `window.resize` on a layer-shell surface asks the compositor for the size
/// and applies it a tick later, so a render that measures a new size has to
/// decide what to paint meanwhile. Before the first fit nothing is painted;
/// after it the content is, because a blink of nothing on every expand would
/// be worse than one frame at the old size.
#[derive(Debug, Default)]
pub struct WindowFit {
    /// The size last asked of the compositor, so an answer it rounds
    /// differently cannot start a resize loop.
    requested: Option<Size<Pixels>>,
    /// Frames rendered since the last request, to give up waiting on a
    /// compositor that never grants exactly what was asked.
    frames_since_request: u8,
    settled: bool,
}

impl WindowFit {
    /// Measure `content` at `width` and bring the window to its height,
    /// capped at `max_height` and the output's usable height less
    /// `vertical_margin`. Returns whether the content should be painted this
    /// frame.
    pub fn fit_height(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        mut content: AnyElement,
        width: Pixels,
        max_height: Pixels,
        vertical_margin: Pixels,
    ) -> bool {
        let measured = content
            .layout_as_root(
                size(AvailableSpace::Definite(width), AvailableSpace::MaxContent),
                window,
                cx,
            )
            .height;

        let mut cap = max_height;
        if let Some(display) = window.display(cx) {
            let usable = display.visible_bounds().size.height - vertical_margin;
            if usable > px(0.) {
                cap = cap.min(usable);
            }
        }
        let desired = size(width, measured.ceil().min(cap).max(px(1.)));
        self.request(window, desired)
    }

    /// Bring the window to `desired`. Returns whether to paint this frame.
    pub fn request(&mut self, window: &mut Window, desired: Size<Pixels>) -> bool {
        if window.viewport_size() == desired {
            self.settled = true;
            self.requested = Some(desired);
            return true;
        }
        if self.requested != Some(desired) {
            self.requested = Some(desired);
            self.frames_since_request = 0;
            window.resize(desired);
        } else {
            self.frames_since_request = self.frames_since_request.saturating_add(1);
            // Asked twice and still not there: the compositor has its own
            // idea of the size, so paint into whatever it gave us.
            if self.frames_since_request >= 2 {
                self.settled = true;
            }
        }
        self.settled
    }

    /// The size the window was last asked to take.
    pub fn requested(&self) -> Option<Size<Pixels>> {
        self.requested
    }
}

/// The window a panel renders in: sizes the window for its content when the
/// panel asked for that, and paints the content once the window fits.
pub struct PanelHost<V: Render> {
    content: Entity<V>,
    size: PanelSize,
    vertical_margin: Pixels,
    fit: WindowFit,
}

impl<V: Render> Render for PanelHost<V> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paint = match self.size {
            PanelSize::Fixed { .. } => true,
            PanelSize::FitHeight { width, max_height } => self.fit.fit_height(
                window,
                cx,
                self.content.clone().into_any_element(),
                px(width),
                px(max_height),
                self.vertical_margin,
            ),
        };
        div()
            .id("panel-host")
            .size_full()
            .when(paint, |el| el.child(self.content.clone()))
    }
}

// ============================================================================
// Registry
// ============================================================================

struct OpenPanel {
    id: String,
    handle: AnyWindowHandle,
}

/// What the shell has open besides its bar, dock and OSD.
#[derive(Default)]
pub struct ShellWindows {
    /// Only one panel is open at a time.
    panel: Option<OpenPanel>,
    launcher: Option<AnyWindowHandle>,
}

impl Global for ShellWindows {}

impl ShellWindows {
    pub fn init(cx: &mut App) {
        cx.set_global(Self::default());
    }

    fn is_alive(cx: &App, handle: AnyWindowHandle) -> bool {
        cx.windows().contains(&handle)
    }

    /// Whether one of our windows took the keyboard: a panel or the launcher
    /// is open. While it is, the compositor reports no focused toplevel, which
    /// says nothing about what is on the desktop.
    pub fn holds_keyboard_focus(cx: &App) -> bool {
        let Some(this) = cx.try_global::<Self>() else {
            return false;
        };
        this.panel
            .as_ref()
            .is_some_and(|panel| Self::is_alive(cx, panel.handle))
            || this
                .launcher
                .is_some_and(|handle| Self::is_alive(cx, handle))
    }

    /// Remember the launcher window; its liveness is checked on every ask, so
    /// a launcher that closed itself needs no bookkeeping.
    pub fn set_launcher(cx: &mut App, handle: Option<AnyWindowHandle>) {
        cx.default_global::<Self>().launcher = handle;
    }

    /// The ID of the open panel, if any.
    #[allow(dead_code)]
    pub fn active_panel_id(cx: &App) -> Option<String> {
        let this = cx.try_global::<Self>()?;
        let panel = this.panel.as_ref()?;
        Self::is_alive(cx, panel.handle).then(|| panel.id.clone())
    }

    /// Whether the panel with this ID is open.
    #[allow(dead_code)]
    pub fn is_panel_open(cx: &App, panel_id: &str) -> bool {
        Self::active_panel_id(cx).as_deref() == Some(panel_id)
    }

    /// Open the panel with this ID, or close it if it is the one open. Any
    /// other open panel closes first. Returns true if the panel was opened.
    pub fn toggle_panel<V: Render + 'static>(
        cx: &mut App,
        panel_id: &str,
        config: PanelConfig,
        build: impl FnOnce(&mut Context<V>) -> V + 'static,
    ) -> bool {
        Self::toggle_panel_on_display(cx, panel_id, config, None, build)
    }

    /// [`Self::toggle_panel`] on an explicitly selected output.
    pub fn toggle_panel_on_display<V: Render + 'static>(
        cx: &mut App,
        panel_id: &str,
        config: PanelConfig,
        display_id: Option<DisplayId>,
        build: impl FnOnce(&mut Context<V>) -> V + 'static,
    ) -> bool {
        if let Some(open) = cx.default_global::<Self>().panel.take() {
            let _ = cx.update_window(open.handle, |_, window, _cx| {
                window.remove_window();
            });
            if open.id == panel_id {
                return false;
            }
        }

        let initial = config.size.initial();
        let vertical_margin = px(config.margin.0 + config.margin.2);
        let window_options = WindowOptions {
            display_id,
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::new(px(0.), px(0.)),
                size: initial,
            })),
            app_id: Some(format!("gpuishell-panel-{}", panel_id)),
            window_background: WindowBackgroundAppearance::Transparent,
            kind: WindowKind::LayerShell(LayerShellOptions {
                namespace: config.namespace,
                layer: Layer::Overlay,
                anchor: config.anchor,
                exclusive_zone: None,
                margin: Some((
                    px(config.margin.0),
                    px(config.margin.1),
                    px(config.margin.2),
                    px(config.margin.3),
                )),
                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                ..Default::default()
            }),
            focus: true,
            ..Default::default()
        };

        let size = config.size;
        let Ok(handle) = cx.open_window(window_options, move |window, cx| {
            window.set_rem_size(cx.theme().font_size);
            cx.new(|cx| PanelHost {
                content: cx.new(build),
                size,
                vertical_margin,
                fit: WindowFit::default(),
            })
        }) else {
            return false;
        };

        cx.default_global::<Self>().panel = Some(OpenPanel {
            id: panel_id.to_string(),
            handle: handle.into(),
        });
        true
    }

    /// Close the open panel, if any.
    pub fn close_panel(cx: &mut App) {
        if let Some(open) = cx.default_global::<Self>().panel.take() {
            let _ = cx.update_window(open.handle, |_, window, _cx| {
                window.remove_window();
            });
        }
    }

    /// Forget a panel that already closed itself through its window lifecycle.
    pub fn forget_panel(cx: &mut App, panel_id: &str) {
        let this = cx.default_global::<Self>();
        if this.panel.as_ref().is_some_and(|open| open.id == panel_id) {
            this.panel = None;
        }
    }
}
