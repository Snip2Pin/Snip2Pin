//! Initial pin placement and controls inside the visible part of the image.
use eframe::egui::{self, Pos2, Rect, Vec2};

use crate::geometry::Placement;

pub struct Metrics {
    pub button: f32,
    pub button_gap: f32,
    /// Keep controls clear of the window edges used for native resizing.
    pub inset: f32,
}

pub const METRICS: Metrics = Metrics {
    button: 26.0,
    button_gap: 4.0,
    inset: 12.0,
};

impl Metrics {
    pub fn toolbar_width(&self) -> f32 {
        self.button * 2.0 + self.button_gap
    }

    pub fn toolbar_height(&self) -> f32 {
        self.button
    }

    pub fn toolbar_size(&self) -> Vec2 {
        egui::vec2(self.toolbar_width(), self.toolbar_height())
    }
}

/// Capture coordinates, native window units and egui zoom are distinct.
pub struct Coordinates {
    pub ratio: f32,
    pub grid: f32,
}

impl Coordinates {
    pub fn new(display_scale: f32, zoom: f32, pixels_per_point: f32) -> Self {
        if cfg!(target_os = "macos") {
            Self {
                ratio: zoom.recip(),
                grid: zoom,
            }
        } else {
            Self {
                ratio: display_scale / pixels_per_point,
                grid: pixels_per_point,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub image: Rect,
    pub controls: Option<Rect>,
}

/// Minimum content size in UI points, including room for resize hit regions.
pub fn minimum_image_size() -> Vec2 {
    METRICS.toolbar_size() + Vec2::splat(2.0 * METRICS.inset)
}

/// Fit the initial image proportionally, enlarging tiny images when space allows.
/// If an extreme aspect ratio cannot fit both the screen and the minimum, clamp
/// the short axis to the native minimum instead of requesting a huge surface.
pub fn frame_geometry(placement: Placement, screen: Rect, ratio: f32, grid: f32) -> Layout {
    let position = Pos2::from(placement.position) * ratio;
    let original = Vec2::from(placement.size) * ratio;
    let area = scale_rect(screen, ratio);
    let fit = (area.size() / original).min_elem();
    let minimum = (minimum_image_size() / original).max_elem();
    let scale = if minimum <= fit {
        minimum.max(1.0).min(fit)
    } else {
        fit.min(1.0)
    };
    let size = (original * scale).max(minimum_image_size());
    let rounded = egui::pos2(
        (position.x * grid).round() / grid,
        (position.y * grid).round() / grid,
    );
    let origin = rounded.clamp(area.min, (area.max - size).max(area.min));
    let image = Rect::from_min_size(origin, size);
    Layout {
        image,
        controls: toolbar_geometry(image, &[], area, 1.0),
    }
}

/// Prefer the image's top-right corner, clamped inside a usable display area.
/// The image is in UI desktop coordinates; monitor bounds are scaled by ratio.
/// Return None only when no display has enough visible image for the controls.
pub fn toolbar_geometry(image: Rect, screens: &[Rect], fallback: Rect, ratio: f32) -> Option<Rect> {
    let size = METRICS.toolbar_size();
    let preferred = image.right_top() + egui::vec2(-METRICS.inset - size.x, METRICS.inset);
    screens
        .iter()
        .copied()
        .map(|screen| scale_rect(screen, ratio))
        .chain(screens.is_empty().then(|| scale_rect(fallback, ratio)))
        .filter_map(|area| {
            let visible = image.intersect(area).shrink(METRICS.inset);
            if visible.width() < size.x || visible.height() < size.y {
                return None;
            }
            let position = preferred.clamp(visible.min, visible.max - size);
            Some(Rect::from_min_size(position, size))
        })
        .min_by(|left, right| {
            left.min
                .distance_sq(preferred)
                .total_cmp(&right.min.distance_sq(preferred))
        })
}

fn scale_rect(rect: Rect, ratio: f32) -> Rect {
    Rect::from_min_max(rect.min * ratio, rect.max * ratio)
}
