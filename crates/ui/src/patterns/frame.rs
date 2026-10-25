//! Screen frame: the bar's background carried around the other three edges,
//! with rounded inner corners where it meets the work area.

use gpui::{
    App, Bounds, FillOptions, FillRule, Hsla, IntoElement, PathBuilder, PathStyle, Pixels,
    RenderOnce, Window, canvas, point, prelude::*, px, size,
};

use super::BarEdge;

/// Paints everything outside the bar's strip and the rounded work-area hole.
/// The bar draws its own strip, so a translucent background never doubles up.
#[derive(IntoElement)]
#[must_use = "ScreenFrame does nothing unless rendered"]
pub struct ScreenFrame {
    edge: BarEdge,
    bar_size: Pixels,
    thickness: Pixels,
    radius: Pixels,
    background: Hsla,
    border: Option<Hsla>,
}

impl ScreenFrame {
    pub fn new(edge: BarEdge, bar_size: Pixels, background: Hsla) -> Self {
        Self {
            edge,
            bar_size,
            thickness: px(0.),
            radius: px(0.),
            background,
            border: None,
        }
    }

    /// Width of the three edges the bar is not on.
    pub fn thickness(mut self, thickness: Pixels) -> Self {
        self.thickness = thickness;
        self
    }

    /// Radius of the hole's corners.
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// Outline the hole.
    pub fn border(mut self, border: Option<Hsla>) -> Self {
        self.border = border;
        self
    }
}

/// `bounds` without the bar's strip on `edge`.
pub fn outside_bar(bounds: Bounds<Pixels>, edge: BarEdge, depth: Pixels) -> Bounds<Pixels> {
    inset(bounds, edge, depth, px(0.))
}

/// The work area the frame leaves open.
pub fn frame_hole(
    bounds: Bounds<Pixels>,
    edge: BarEdge,
    bar_size: Pixels,
    thickness: Pixels,
) -> Bounds<Pixels> {
    inset(bounds, edge, bar_size, thickness)
}

/// Inset `bounds` by `along` on `edge` and by `other` on the remaining three.
fn inset(bounds: Bounds<Pixels>, edge: BarEdge, along: Pixels, other: Pixels) -> Bounds<Pixels> {
    let pick = |e: BarEdge| if e == edge { along } else { other };
    let (top, right, bottom, left) = (
        pick(BarEdge::Top),
        pick(BarEdge::Right),
        pick(BarEdge::Bottom),
        pick(BarEdge::Left),
    );
    Bounds {
        origin: point(bounds.origin.x + left, bounds.origin.y + top),
        size: size(
            (bounds.size.width - left - right).max(px(0.)),
            (bounds.size.height - top - bottom).max(px(0.)),
        ),
    }
}

fn rounded_rect(builder: &mut PathBuilder, rect: Bounds<Pixels>, radius: Pixels) {
    let r = radius
        .min(rect.size.width / 2.)
        .min(rect.size.height / 2.)
        .max(px(0.));
    let radii = point(r, r);
    let (left, top) = (rect.origin.x, rect.origin.y);
    let (right, bottom) = (left + rect.size.width, top + rect.size.height);

    builder.move_to(point(left + r, top));
    builder.line_to(point(right - r, top));
    builder.arc_to(radii, px(0.), false, true, point(right, top + r));
    builder.line_to(point(right, bottom - r));
    builder.arc_to(radii, px(0.), false, true, point(right - r, bottom));
    builder.line_to(point(left + r, bottom));
    builder.arc_to(radii, px(0.), false, true, point(left, bottom - r));
    builder.line_to(point(left, top + r));
    builder.arc_to(radii, px(0.), false, true, point(left + r, top));
    builder.close();
}

impl RenderOnce for ScreenFrame {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let outer = outside_bar(bounds, self.edge, self.bar_size);
                let hole = frame_hole(bounds, self.edge, self.bar_size, self.thickness);
                if outer.size.width <= px(0.) || outer.size.height <= px(0.) {
                    return;
                }

                // Even-odd turns the hole into a cut-out of the outer rect.
                let mut fill = PathBuilder::fill().with_style(PathStyle::Fill(
                    FillOptions::default().with_fill_rule(FillRule::EvenOdd),
                ));
                rounded_rect(&mut fill, outer, px(0.));
                rounded_rect(&mut fill, hole, self.radius);
                if let Ok(path) = fill.build() {
                    window.paint_path(path, self.background);
                }

                if let Some(color) = self.border {
                    // Centre the hairline on the hole's edge pixel, not between two.
                    let line = Bounds {
                        origin: point(hole.origin.x + px(0.5), hole.origin.y + px(0.5)),
                        size: size(hole.size.width - px(1.), hole.size.height - px(1.)),
                    };
                    let mut stroke = PathBuilder::stroke(px(1.));
                    rounded_rect(&mut stroke, line, self.radius - px(0.5));
                    if let Ok(path) = stroke.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1000.), px(600.)),
        }
    }

    #[test]
    fn the_hole_takes_the_bar_on_its_edge_and_the_thickness_elsewhere() {
        let hole = frame_hole(screen(), BarEdge::Left, px(40.), px(6.));
        assert_eq!(hole.origin, point(px(40.), px(6.)));
        assert_eq!(hole.size, size(px(954.), px(588.)));

        let hole = frame_hole(screen(), BarEdge::Bottom, px(32.), px(4.));
        assert_eq!(hole.origin, point(px(4.), px(4.)));
        assert_eq!(hole.size, size(px(992.), px(564.)));
    }

    #[test]
    fn outside_bar_is_the_screen_minus_one_strip() {
        let rest = outside_bar(screen(), BarEdge::Top, px(48.));
        assert_eq!(rest.origin, point(px(0.), px(48.)));
        assert_eq!(rest.size, size(px(1000.), px(552.)));
    }
}
