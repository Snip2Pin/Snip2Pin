/// A validated rectangle in physical image pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Convert any drag direction from viewport points to image pixels. Ratios are
/// derived from the actual viewport, avoiding double application of Retina DPI.
pub fn selection(
    start: [f64; 2],
    end: [f64; 2],
    viewport: [f64; 2],
    pixels: [u32; 2],
) -> Option<PixelRect> {
    if !start
        .into_iter()
        .chain(end)
        .chain(viewport)
        .all(f64::is_finite)
        || viewport.into_iter().any(|v| v <= 0.0)
        || pixels.into_iter().any(|v| v == 0)
    {
        return None;
    }
    let mut low = [0; 2];
    let mut high = [0; 2];
    for axis in 0..2 {
        let ratio = f64::from(pixels[axis]) / viewport[axis];
        low[axis] = (start[axis].min(end[axis]).clamp(0.0, viewport[axis]) * ratio).floor() as u32;
        high[axis] = ((start[axis].max(end[axis]).clamp(0.0, viewport[axis]) * ratio).ceil()
            as u32)
            .min(pixels[axis]);
    }
    let width = high[0].saturating_sub(low[0]);
    let height = high[1].saturating_sub(low[1]);
    (width >= 2 && height >= 2).then_some(PixelRect {
        x: low[0],
        y: low[1],
        width,
        height,
    })
}

pub fn fit_size(pixels: [u32; 2], scale: f32, maximum: [f32; 2]) -> [f32; 2] {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let size = [pixels[0] as f32 / scale, pixels[1] as f32 / scale];
    let ratio = (maximum[0] / size[0]).min(maximum[1] / size[1]).min(1.0);
    [(size[0] * ratio).max(1.0), (size[1] * ratio).max(1.0)]
}
