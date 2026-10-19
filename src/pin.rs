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

/// Explicit adjustments preserve proportions; Shift selects the faster pace.
pub const FINE_ZOOM_FACTOR: f32 = 1.01;
pub const FAST_ZOOM_FACTOR: f32 = 1.25;
pub const FAST_MOVE_STEPS: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    In,
    Out,
    Original,
    Scale(f32),
}

/// Size in native window logical units, derived from its observed content size.
/// `pixels_per_point` is the native display scale; the host converts egui zoom
/// at its boundary. macOS windows quantize to whole logical points, so
/// zoom-in steps use one point there, one physical pixel elsewhere, so tiny
/// pins can grow again. Original requests source pixels / display scale (100%).
/// The window system owns native size constraints.
pub fn zoom_size(
    current: Vec2,
    original_pixels: [usize; 2],
    pixels_per_point: f32,
    command: Zoom,
) -> Option<Vec2> {
    if !current.is_finite() || current.min_elem() <= 0.0 || original_pixels.contains(&0) {
        return None;
    }
    let growth_step = native_step(pixels_per_point)?;
    let growth_factor = growth_step / current.min_elem();
    let size = match command {
        // A one-pixel short edge must grow on the next native size request,
        // rather than rounding a small increase back to the same pixel.
        Zoom::In => current * FINE_ZOOM_FACTOR.max(1.0 + growth_factor),
        Zoom::Out => current * FINE_ZOOM_FACTOR.recip(),
        Zoom::Scale(factor) if factor.is_finite() && factor > 0.0 => {
            current
                * if factor > 1.0 {
                    factor.max(1.0 + growth_factor)
                } else {
                    factor
                }
        }
        Zoom::Scale(_) => return None,
        Zoom::Original => {
            egui::vec2(original_pixels[0] as f32, original_pixels[1] as f32) / pixels_per_point
        }
    };
    (size.is_finite() && size.min_elem() > 0.0).then_some(size)
}

/// Convert accumulated arrow-key steps into native logical window units.
/// macOS uses one whole logical point per step; other hosts use one physical
/// pixel. The caller obtains the current native origin before applying this
/// delta, without keeping another position or compensating for system limits.
pub fn movement_delta(steps: [i32; 2], native_scale: f32) -> Option<Vec2> {
    if steps == [0, 0] {
        return None;
    }
    let delta = egui::vec2(steps[0] as f32, steps[1] as f32) * native_step(native_scale)?;
    (delta.is_finite() && delta != Vec2::ZERO).then_some(delta)
}

fn native_step(native_scale: f32) -> Option<f32> {
    if !native_scale.is_finite() || native_scale <= 0.0 {
        return None;
    }
    let step = if cfg!(target_os = "macos") {
        1.0
    } else {
        native_scale.recip()
    };
    (step.is_finite() && step > 0.0).then_some(step)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Copy,
    Lock,
    Close,
}

impl Control {
    pub const ALL: [Self; 3] = [Self::Copy, Self::Lock, Self::Close];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Toolbar {
    pub bounds: Rect,
    pub count: usize,
}

impl Toolbar {
    pub fn translate(self, offset: Vec2) -> Self {
        Self {
            bounds: self.bounds.translate(offset),
            ..self
        }
    }
}

impl Metrics {
    pub fn toolbar_width(&self) -> f32 {
        self.toolbar_size_for(Control::ALL.len()).x
    }

    pub fn toolbar_height(&self) -> f32 {
        self.button
    }

    pub fn toolbar_size(&self) -> Vec2 {
        egui::vec2(self.toolbar_width(), self.toolbar_height())
    }

    pub fn toolbar_size_for(&self, count: usize) -> Vec2 {
        egui::vec2(
            self.button * count as f32 + self.button_gap * count.saturating_sub(1) as f32,
            self.button,
        )
    }
}

/// Capture coordinates, native window units and egui zoom are distinct.
pub fn coordinate_ratio(display_scale: f32, zoom: f32, pixels_per_point: f32) -> f32 {
    if cfg!(target_os = "macos") {
        zoom.recip()
    } else {
        display_scale / pixels_per_point
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub image: Rect,
    pub controls: Option<Toolbar>,
}

/// Preserve the initial image size; only shrink proportionally to fit the screen.
/// Controls adapt to the image and must never determine its dimensions.
pub fn frame_geometry(placement: Placement, screen: Rect, ratio: f32) -> Layout {
    let position = Pos2::from(placement.position) * ratio;
    let original = Vec2::from(placement.size) * ratio;
    let area = scale_rect(screen, ratio);
    let size = original * (area.size() / original).min_elem().min(1.0);
    let origin = position.clamp(area.min, (area.max - size).max(area.min));
    let image = Rect::from_min_size(origin, size);
    Layout {
        image,
        controls: toolbar_geometry(image, &[], area, 1.0),
    }
}

/// Keep a popup in the clicked display using the originating viewport's units.
pub fn menu_position(
    anchor: Pos2,
    size: Vec2,
    screens: &[Rect],
    fallback: Rect,
    ratio: f32,
) -> Pos2 {
    let area = screens
        .iter()
        .copied()
        .find(|area| scale_rect(*area, ratio).contains(anchor))
        .unwrap_or(fallback);
    let area = scale_rect(area, ratio);
    anchor.clamp(area.min, (area.max - size).max(area.min))
}

/// Prefer the image's top-right corner, clamped inside a usable display area.
/// The image is in UI desktop coordinates; monitor bounds are scaled by ratio.
/// Return None only when no display has enough visible image for the controls.
pub fn toolbar_geometry(
    image: Rect,
    screens: &[Rect],
    fallback: Rect,
    ratio: f32,
) -> Option<Toolbar> {
    for count in (1..=Control::ALL.len()).rev() {
        let size = METRICS.toolbar_size_for(count);
        let preferred = image.right_top() + egui::vec2(-METRICS.inset - size.x, METRICS.inset);
        let candidate = screens
            .iter()
            .copied()
            .map(|screen| scale_rect(screen, ratio))
            .chain(screens.is_empty().then(|| scale_rect(fallback, ratio)))
            .filter_map(|area| {
                let visible = image.intersect(area);
                let magnitude = visible
                    .min
                    .to_vec2()
                    .abs()
                    .max(visible.max.to_vec2().abs())
                    .max(size);
                // Allow only one representable coordinate step on each axis.
                let roundoff = egui::vec2(
                    magnitude.x.next_up() - magnitude.x,
                    magnitude.y.next_up() - magnitude.y,
                );
                if visible.width() + roundoff.x < size.x || visible.height() + roundoff.y < size.y {
                    return None;
                }
                let inset = ((visible.size() - size) * 0.5)
                    .max(Vec2::ZERO)
                    .min(Vec2::splat(METRICS.inset));
                let available = Rect::from_min_max(visible.min + inset, visible.max - inset);
                let position =
                    preferred.clamp(available.min, (available.max - size).max(available.min));
                Some(Toolbar {
                    bounds: Rect::from_min_size(position, size),
                    count,
                })
            })
            .min_by(|left, right| {
                left.bounds
                    .min
                    .distance_sq(preferred)
                    .total_cmp(&right.bounds.min.distance_sq(preferred))
            });
        if candidate.is_some() {
            return candidate;
        }
    }
    None
}

fn scale_rect(rect: Rect, ratio: f32) -> Rect {
    Rect::from_min_max(rect.min * ratio, rect.max * ratio)
}
