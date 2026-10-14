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
    let mut frame = image_frame(position, size, scale_rect(screen, ratio), grid);
    frame.toolbar_side = toolbar_side;
    frame.top_edge = top_edge;
    frame
}

fn image_frame(position: Pos2, size: Vec2, area: Rect, grid: f32) -> Frame {
    let vertical_margin = METRICS.padding + METRICS.toolbar_height();
    let horizontal_margin = METRICS.padding + METRICS.toolbar_width() + METRICS.image_gap;
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
        controls_area: area,
        grid,
        toolbar_side: ToolbarSide::Top,
        toolbar_offset: Vec2::ZERO,
        top_edge: false,
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
    /// Rebuild margins and controls around the desired image position, without
    /// reapplying initial-placement adjustments or changing the image dimensions.
    pub fn moved_to(&self, image_origin: Pos2, screens: &[Rect], ratio: f32) -> Self {
        let area = nearest_area(
            image_origin + self.image.size() / 2.0,
            screens,
            self.controls_area,
            ratio,
        );
        let mut frame = image_frame(image_origin, self.image.size(), area, self.grid);
        frame.place_controls(frame.window, &[], area, 1.0);
        frame
    }

    pub fn resized_to(&self, image: Rect, screens: &[Rect], ratio: f32) -> Self {
        let mut frame = *self;
        frame.image = Rect::from_min_size(frame.image.min, image.size());
        frame.moved_to(image.min, screens, ratio)
    }

    /// Keep the whole image below AppKit's work-area boundary, and reduce only
    /// transparent upper margins. The picture can touch the menu bar without
    /// being cropped, scaled, or leaving its native window.
    pub fn fit_host_top(&mut self) {
        let image = self.image.translate(self.window.min.to_vec2());
        let mut position = egui::pos2(image.min.x, image.min.y.max(self.controls_area.min.y));
        let top_edge = self.top_edge;
        *self = self.moved_to(position, &[], 1.0);
        let (copy, close) = self.toolbar_rects();
        let row = copy.union(close).translate(self.window.min.to_vec2());
        // A screen-sized image can leave no room below or beside it. Keep a
        // reachable upper row in that case, preserving every image pixel.
        if !self.controls_area.contains_rect(row) {
            position.y = position
                .y
                .max(self.controls_area.min.y + METRICS.toolbar_height());
            *self = self.moved_to(position, &[], 1.0);
        }
        self.top_edge = top_edge;
        let top = self.window.min.y.max(self.controls_area.min.y);
        let shift = egui::vec2(0.0, self.window.min.y - top);
        self.window.min.y = top;
        self.image = self.image.translate(shift);
    }

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
    pub resize: Resize,
    size_native: Option<Vec2>,
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
        if let Some(size) = self.size_native {
            frame.image = Rect::from_min_size(frame.image.min, size / frame.grid);
        }
        let offset = *self.image_offset.get_or_insert_with(|| {
            let mut offset = inner.map_or(Vec2::ZERO, |rect| frame.window.min - rect.min);
            if frame.top_edge {
                offset.y = 0.0;
            }
            offset / ratio
        });
        frame.image = if let Some(position) = self.drag.position(frame.grid) {
            Rect::from_min_size(
                (position - inner.unwrap_or(frame.window).min).to_pos2(),
                frame.image.size(),
            )
        } else {
            frame.image.translate(offset * ratio)
        };
        Prepared::Paint { frame, first }
    }

    pub fn set_size(&mut self, size: Vec2, grid: f32) {
        self.size_native = Some(size * grid);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];
    pub fn point(self, rect: Rect) -> Pos2 {
        match self {
            Self::TopLeft => rect.left_top(),
            Self::TopRight => rect.right_top(),
            Self::BottomLeft => rect.left_bottom(),
            Self::BottomRight => rect.right_bottom(),
        }
    }
    fn opposite(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomLeft => Self::TopRight,
            Self::BottomRight => Self::TopLeft,
        }
    }
    fn direction(self) -> Vec2 {
        match self {
            Self::TopLeft => egui::vec2(-1.0, -1.0),
            Self::TopRight => egui::vec2(1.0, -1.0),
            Self::BottomLeft => egui::vec2(-1.0, 1.0),
            Self::BottomRight => Vec2::splat(1.0),
        }
    }
}

struct ResizeAnchor {
    corner: Corner,
    image: Rect,
    pointer: Pos2,
}

#[derive(Default)]
pub struct Resize {
    anchor: Option<ResizeAnchor>,
}

impl Resize {
    pub fn begin(&mut self, corner: Corner, image: Rect, pointer: Pos2) {
        self.anchor = Some(ResizeAnchor {
            corner,
            image,
            pointer,
        });
    }
    /// Native coordinates keep the opposite corner and grabbed pixel stable
    /// across DPI changes. Shift locks the proportions at gesture start.
    pub fn update(
        &self,
        pointer: Pos2,
        proportional: bool,
        minimum: Vec2,
        top: Option<f32>,
    ) -> Option<Rect> {
        let anchor = self.anchor.as_ref()?;
        let direction = anchor.corner.direction();
        let delta = (pointer - anchor.pointer) * direction;
        let mut size = (anchor.image.size() + delta).max(minimum);
        let limit = if direction.y < 0.0 {
            top.map(|top| (anchor.image.max.y - top).max(minimum.y))
        } else {
            None
        };
        if proportional {
            let relative = delta / anchor.image.size();
            let factor = if relative.x.abs() >= relative.y.abs() {
                1.0 + relative.x
            } else {
                1.0 + relative.y
            };
            let minimum_factor = (minimum / anchor.image.size()).max_elem();
            let factor = factor.max(minimum_factor);
            let factor = limit.map_or(factor, |height| factor.min(height / anchor.image.height()));
            size = anchor.image.size() * factor;
        } else if let Some(height) = limit {
            size.y = size.y.min(height);
        }
        let pivot = anchor.corner.opposite().point(anchor.image);
        let point = pivot + size * direction;
        Some(Rect::from_min_max(pivot.min(point), pivot.max(point)))
    }
    pub fn is_active(&self) -> bool {
        self.anchor.is_some()
    }
    pub fn end(&mut self) {
        self.anchor = None;
    }
}

/// Anchor dragging to the image, so switching transparent margins cannot change
/// the grabbed pixel. Retain the image position after the user releases it.
#[derive(Default)]
pub struct Drag {
    anchor_native: Option<Vec2>,
    last_requested_native: Option<Pos2>,
    image_position_native: Option<Pos2>,
}

impl Drag {
    pub fn begin(&mut self, offset: Vec2, origin: Pos2, coordinates: &Coordinates) {
        self.anchor_native = Some(offset * coordinates.cursor_scale);
        self.last_requested_native = Some(origin * coordinates.cursor_scale);
        self.image_position_native = self.last_requested_native;
    }

    pub fn move_to(&mut self, cursor: [f32; 2], coordinates: &Coordinates) -> Option<Pos2> {
        let position = Pos2::from(cursor) - self.anchor_native?;
        if self.last_requested_native == Some(position) {
            return None;
        }
        self.last_requested_native = Some(position);
        self.image_position_native = Some(position);
        Some(coordinates.cursor(position.into()))
    }

    pub fn is_active(&self) -> bool {
        self.anchor_native.is_some()
    }

    pub fn position(&self, grid: f32) -> Option<Pos2> {
        self.image_position_native.map(|position| position / grid)
    }

    pub fn set_position(&mut self, position: Pos2, coordinates: &Coordinates) {
        self.image_position_native = Some(position * coordinates.cursor_scale);
    }

    pub fn end(&mut self) {
        self.anchor_native = None;
    }
}
