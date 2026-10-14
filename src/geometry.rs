/// A validated rectangle in physical image pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

const MIN_SELECTION_EDGE: u32 = 2;

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
    (width >= MIN_SELECTION_EDGE && height >= MIN_SELECTION_EDGE).then_some(PixelRect {
        x: low[0],
        y: low[1],
        width,
        height,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: [f32; 2],
    pub size: [f32; 2],
}

/// Preserve the selected region's desktop position and displayed size. Image
/// dimensions and screen points are independent (Retina, mixed DPI, demo data).
pub fn pin_placement(
    rect: PixelRect,
    pixels: [u32; 2],
    screen_origin: [f32; 2],
    screen_size: [f32; 2],
) -> Placement {
    let ratio = [
        screen_size[0] / pixels[0] as f32,
        screen_size[1] / pixels[1] as f32,
    ];
    Placement {
        position: [
            screen_origin[0] + rect.x as f32 * ratio[0],
            screen_origin[1] + rect.y as f32 * ratio[1],
        ],
        size: [rect.width as f32 * ratio[0], rect.height as f32 * ratio[1]],
    }
}
