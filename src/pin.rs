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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarSide {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub struct Frame {
    pub window: Rect,
    pub image: Rect,
    pub controls_area: Rect,
    pub grid: f32,
    pub toolbar_side: ToolbarSide,
    pub toolbar_offset: Vec2,
    pub top_edge: bool,
}

pub fn frame_geometry(placement: Placement, screen: Rect, ratio: f32, grid: f32) -> Frame {
    let mut position = Pos2::from(placement.position) * ratio;
    let size = Vec2::from(placement.size) * ratio;
    let screen_top = (screen.min.y * ratio * grid).ceil() / grid;
    let vertical_margin = METRICS.padding + METRICS.toolbar_height();
    let horizontal_margin = METRICS.padding + METRICS.toolbar_width() + METRICS.image_gap;
    let top_edge = position.y - vertical_margin < screen_top;
    let mut toolbar_side = ToolbarSide::Top;
    if top_edge {
        position.y = position.y.max(screen_top + METRICS.padding);
        toolbar_side = ToolbarSide::Bottom;
        if position.y + size.y + vertical_margin > screen.max.y * ratio {
            toolbar_side = ToolbarSide::Top;
            position.y = position.y.max(screen_top + vertical_margin);
        }
    }
    let origin = egui::pos2(
        ((position.x - horizontal_margin) * grid).floor() / grid,
        ((position.y - vertical_margin) * grid).floor() / grid,
    );
    let image = Rect::from_min_size((position - origin).to_pos2(), size);
    let window_size = egui::vec2(
        ((image.min.x + size.x.max(METRICS.toolbar_width()) + horizontal_margin) * grid).ceil()
            / grid,
        ((image.max.y + vertical_margin) * grid).ceil() / grid,
    );
    Frame {
        window: Rect::from_min_size(origin, window_size),
        image,
        controls_area: scale_rect(screen, ratio),
        grid,
        toolbar_side,
        toolbar_offset: Vec2::ZERO,
        top_edge,
    }
}

fn toolbar_row(image: Rect, side: ToolbarSide) -> Rect {
    let width = METRICS.toolbar_width();
    let position = match side {
        ToolbarSide::Top => egui::pos2(
            image.max.x.max(image.min.x + width) - width,
            image.min.y - METRICS.toolbar_height(),
        ),
        ToolbarSide::Bottom => egui::pos2(
            image.max.x.max(image.min.x + width) - width,
            image.max.y + METRICS.image_gap,
        ),
        ToolbarSide::Left => egui::pos2(image.min.x - METRICS.image_gap - width, image.min.y),
        ToolbarSide::Right => egui::pos2(image.max.x + METRICS.image_gap, image.min.y),
    };
    Rect::from_min_size(position, egui::vec2(width, METRICS.button))
}

fn nearest_area(point: Pos2, screens: &[Rect], fallback: Rect, ratio: f32) -> Rect {
    screens
        .iter()
        .copied()
        .map(|area| scale_rect(area, ratio))
        .min_by(|left, right| {
            left.distance_to_pos(point)
                .total_cmp(&right.distance_to_pos(point))
        })
        .unwrap_or(fallback)
}

impl Frame {
    /// Stop only when the row on the opposite side would leave the work area.
    /// Choose a monitor from the proposed position, so gaps and negative origins
    /// do not trap a pin on its previous display.
    pub fn drag_position(&self, outer: Pos2, inset: Vec2, screens: &[Rect], ratio: f32) -> Pos2 {
        let candidate = self.image.translate((outer + inset).to_vec2());
        let area = nearest_area(candidate.center(), screens, self.controls_area, ratio);
        let minimum = area.min - self.image.max.to_vec2() - Vec2::splat(METRICS.image_gap) - inset;
        let maximum = area.max - self.image.min.to_vec2() + Vec2::splat(METRICS.image_gap) - inset;
        // Account for native rounding before moving, so an offscreen row is
        // never "fixed" by retrying after AppKit/winit reports a rounded origin.
        let minimum = egui::pos2(
            (minimum.x * self.grid).ceil(),
            (minimum.y * self.grid).ceil(),
        ) / self.grid;
        let maximum = egui::pos2(
            (maximum.x * self.grid).floor(),
            (maximum.y * self.grid).floor(),
        ) / self.grid;
        let position =
            egui::pos2((outer.x * self.grid).round(), (outer.y * self.grid).round()) / self.grid;
        position.clamp(minimum, maximum)
    }

    pub fn toolbar_rects(&self) -> (Rect, Rect) {
        let row = toolbar_row(self.image, self.toolbar_side).translate(self.toolbar_offset);
        let copy = Rect::from_min_size(row.min, Vec2::splat(METRICS.button));
        let close = copy.translate(egui::vec2(METRICS.button + METRICS.button_gap, 0.0));
        (copy, close)
    }

    pub fn place_controls(&mut self, inner: Rect, screens: &[Rect], fallback: Rect, ratio: f32) {
        let image = self.image.translate(inner.min.to_vec2());
        self.controls_area =
            nearest_area(image.center(), screens, scale_rect(fallback, ratio), ratio);
        use ToolbarSide::{Bottom, Left, Right, Top};
        let candidates = if image.max.x > self.controls_area.max.x {
            [Left, Bottom, Top, Right]
        } else if image.min.x < self.controls_area.min.x {
            [Right, Bottom, Top, Left]
        } else if image.min.y - METRICS.toolbar_height() < self.controls_area.min.y {
            [Bottom, Left, Right, Top]
        } else {
            [Top, Bottom, Left, Right]
        };
        for side in candidates {
            let mut row = toolbar_row(image, side);
            let maximum = (self.controls_area.max - row.size()).max(self.controls_area.min);
            row = Rect::from_min_size(row.min.clamp(self.controls_area.min, maximum), row.size());
            if self.controls_area.contains_rect(row)
                && inner.contains_rect(row)
                && !row.intersects(image)
            {
                self.toolbar_side = side;
                self.toolbar_offset =
                    row.min - inner.min.to_vec2() - toolbar_row(self.image, side).min;
                return;
            }
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
