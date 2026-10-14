//! Pin geometry and user-driven movement, independent of native windows.
use eframe::egui::{self, Pos2, Rect, Vec2};

use crate::geometry::Placement;

pub struct Metrics {
    pub padding: f32,
    pub button: f32,
    pub button_gap: f32,
    pub image_gap: f32,
}

pub const METRICS: Metrics = Metrics {
    padding: 10.0,
    button: 26.0,
    button_gap: 4.0,
    image_gap: 6.0,
};

impl Metrics {
    pub fn toolbar_height(&self) -> f32 {
        self.button + self.image_gap
    }

    fn toolbar_width(&self) -> f32 {
        self.button * 2.0 + self.button_gap
    }
}

/// Capture coordinates, native window units and egui zoom are distinct.
pub struct Coordinates {
    pub ratio: f32,
    pub grid: f32,
    cursor_scale: f32,
}

impl Coordinates {
    pub fn new(display_scale: f32, zoom: f32, pixels_per_point: f32) -> Self {
        if cfg!(target_os = "macos") {
            Self {
                ratio: zoom.recip(),
                grid: zoom,
                cursor_scale: zoom,
            }
        } else {
            Self {
                ratio: display_scale / pixels_per_point,
                grid: pixels_per_point,
                cursor_scale: pixels_per_point,
            }
        }
    }

    pub fn cursor(&self, point: [f32; 2]) -> Pos2 {
        Pos2::from(point) / self.cursor_scale
    }
}

#[derive(Clone, Copy)]
pub struct Frame {
    pub window: Rect,
    pub image: Rect,
    pub controls_area: Rect,
    pub toolbar_below: bool,
    pub top_edge: bool,
}

pub fn frame_geometry(placement: Placement, screen: Rect, ratio: f32, grid: f32) -> Frame {
    let mut position = Pos2::from(placement.position) * ratio;
    let size = Vec2::from(placement.size) * ratio;
    let screen_top = (screen.min.y * ratio * grid).ceil() / grid;
    let row = METRICS.toolbar_height();
    let top_edge = position.y - METRICS.padding - row < screen_top;
    let mut toolbar_below = top_edge;
    if top_edge {
        position.y = position.y.max(screen_top + METRICS.padding);
        if position.y + size.y + METRICS.padding + row > screen.max.y * ratio {
            toolbar_below = false;
            position.y = position.y.max(screen_top + METRICS.padding + row);
        }
    }
    let origin = egui::pos2(
        ((position.x - METRICS.padding) * grid).floor() / grid,
        ((position.y - METRICS.padding - row) * grid).floor() / grid,
    );
    let image = Rect::from_min_size((position - origin).to_pos2(), size);
    let window_size = egui::vec2(
        ((image.min.x + size.x.max(METRICS.toolbar_width()) + METRICS.padding) * grid).ceil()
            / grid,
        ((image.max.y + METRICS.padding + row) * grid).ceil() / grid,
    );
    Frame {
        window: Rect::from_min_size(origin, window_size),
        image,
        controls_area: scale_rect(screen, ratio),
        toolbar_below,
        top_edge,
    }
}

pub fn toolbar_below(window: Rect, screen: Rect) -> bool {
    window.min.y - METRICS.toolbar_height() < screen.min.y
}

impl Frame {
    /// Allow the picture past the top, stopping only when its lower controls
    /// reach the display edge. Positions and native insets are viewport points.
    pub fn upward_drag_position(&self, outer: Pos2, inset: Vec2) -> Pos2 {
        let lower_toolbar_top = self.image.max.y + METRICS.image_gap;
        egui::pos2(
            outer.x,
            outer
                .y
                .max(self.controls_area.min.y - lower_toolbar_top - inset.y),
        )
    }

    pub fn toolbar_rects(&self) -> (Rect, Rect) {
        let close = Rect::from_min_size(
            egui::pos2(
                self.image
                    .max
                    .x
                    .max(self.image.min.x + METRICS.toolbar_width())
                    - METRICS.button,
                if self.toolbar_below {
                    self.image.max.y + METRICS.image_gap
                } else {
                    self.image.min.y - METRICS.toolbar_height()
                },
            ),
            Vec2::splat(METRICS.button),
        );
        let copy = close.translate(egui::vec2(-METRICS.button_gap - METRICS.button, 0.0));
        (copy, close)
    }

    pub fn place_controls(&mut self, inner: Rect, screens: &[Rect], fallback: Rect, ratio: f32) {
        let image = self.image.translate(inner.min.to_vec2());
        self.controls_area = screens
            .iter()
            .copied()
            .map(|area| scale_rect(area, ratio))
            .min_by(|left, right| {
                left.distance_to_pos(image.center())
                    .total_cmp(&right.distance_to_pos(image.center()))
            })
            .unwrap_or(scale_rect(fallback, ratio));
        self.toolbar_below = toolbar_below(inner, self.controls_area);
        let row = METRICS.toolbar_height();
        if self.toolbar_below
            && image.max.y + row > self.controls_area.max.y
            && image.min.y - row >= self.controls_area.min.y
        {
            self.toolbar_below = false;
        }
    }
}

fn scale_rect(rect: Rect, ratio: f32) -> Rect {
    Rect::from_min_max(rect.min * ratio, rect.max * ratio)
}

#[derive(Default)]
pub struct Preparation {
    requested: bool,
    image_offset: Option<Vec2>,
    pub drag: Drag,
}

pub enum Prepared {
    Initialize { window: Rect, outer_position: Pos2 },
    Paint { frame: Frame, first: bool },
}

impl Preparation {
    /// Initialize once; compensate native rounding in the image, never retry.
    pub fn update(
        &mut self,
        mut frame: Frame,
        inner: Option<Rect>,
        outer: Option<Rect>,
        ratio: f32,
    ) -> Prepared {
        if !self.requested {
            self.requested = true;
            let inset = inner
                .zip(outer)
                .map_or(Vec2::ZERO, |(inner, outer)| inner.min - outer.min);
            return Prepared::Initialize {
                window: frame.window,
                outer_position: frame.window.min - inset,
            };
        }
        let first = self.image_offset.is_none();
        let offset = *self.image_offset.get_or_insert_with(|| {
            let mut offset = inner.map_or(Vec2::ZERO, |rect| frame.window.min - rect.min);
            if frame.top_edge {
                offset.y = 0.0;
            }
            offset / ratio
        });
        frame.image = frame.image.translate(offset * ratio);
        Prepared::Paint { frame, first }
    }
}

/// Absolute pointer anchoring avoids native drags constraining transparent
/// margins to the work area. Only pointer movement generates a new position.
#[derive(Default)]
pub struct Drag {
    anchor_native: Option<Vec2>,
    last_position_native: Option<Pos2>,
}

impl Drag {
    pub fn begin(&mut self, press: Pos2, inner: Rect, outer: Rect, coordinates: &Coordinates) {
        self.anchor_native =
            Some((press.to_vec2() + (inner.min - outer.min)) * coordinates.cursor_scale);
        self.last_position_native = Some(outer.min * coordinates.cursor_scale);
    }

    pub fn move_to(&mut self, cursor: [f32; 2], coordinates: &Coordinates) -> Option<Pos2> {
        let position = Pos2::from(cursor) - self.anchor_native?;
        if self.last_position_native == Some(position) {
            return None;
        }
        self.last_position_native = Some(position);
        Some(coordinates.cursor(position.into()))
    }

    pub fn is_active(&self) -> bool {
        self.anchor_native.is_some()
    }

    pub fn end(&mut self) {
        *self = Self::default();
    }
}
