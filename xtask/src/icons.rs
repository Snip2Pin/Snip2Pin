//! Generate native icon assets without linking SVG rendering into the application.
use crate::Result;

const ICONS: [(&str, u32); 3] = [("line", 1024), ("metal", 1024), ("status", 32)];
const WINDOWS_ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const WINDOWS_TRAY_ICON_SIZES: [u32; 8] = [16, 20, 24, 28, 32, 40, 48, 64];
// A single mid-luminance green remains distinct on light and dark taskbars.
// Override only the shared status artwork's ink; keep its geometry unchanged.
const WINDOWS_TRAY_STYLE: &str = "g { stroke: #478C72; }";

pub fn generate() -> Result {
    for (name, size) in ICONS {
        render(
            &std::fs::read_to_string(format!("assets/icons/{name}.svg"))?,
            size,
            None,
        )?
        .save(format!("assets/icons/{name}.png"))?;
    }
    let svg = std::fs::read_to_string("assets/icons/line.svg")?;
    std::fs::write(
        "assets/icons/line.ico",
        windows_icon(&svg, &WINDOWS_ICON_SIZES, None)?,
    )?;
    let svg = std::fs::read_to_string("assets/icons/status.svg")?;
    std::fs::write(
        "assets/icons/status-windows.ico",
        windows_icon(&svg, &WINDOWS_TRAY_ICON_SIZES, Some(WINDOWS_TRAY_STYLE))?,
    )?;
    Ok(())
}

fn windows_icon(svg: &str, sizes: &[u32], style: Option<&str>) -> Result<Vec<u8>> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};

    let frames = sizes
        .iter()
        .map(|&size| {
            let pixels = render(svg, size, style)?;
            Ok(IcoFrame::as_png(
                &pixels,
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )?)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut bytes = Vec::new();
    IcoEncoder::new(&mut bytes).encode_images(&frames)?;
    Ok(bytes)
}

fn render(svg: &str, size: u32, style: Option<&str>) -> Result<image::RgbaImage> {
    let options = resvg::usvg::Options {
        style_sheet: style.map(str::to_owned),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("Invalid icon size")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            size as f32 / tree.size().width(),
            size as f32 / tree.size().height(),
        ),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let pixel = pixel.demultiply();
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(size, size, rgba).ok_or_else(|| "Invalid icon pixels".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_pngs_exactly_match_vector_sources() {
        let root = crate::root().expect("runtime workspace");
        for (name, size) in ICONS {
            let stem = root.join("assets/icons").join(name);
            let svg = std::fs::read_to_string(stem.with_extension("svg")).expect("SVG source");
            let png = image::open(stem.with_extension("png"))
                .expect("LFS PNG")
                .to_rgba8();
            assert_eq!(png.dimensions(), (size, size), "{name} dimensions");
            assert!(
                png == render(&svg, size, None).expect("SVG rasterization"),
                "{name} PNG differs from its SVG source; run cargo xtask icons"
            );
        }
    }

    #[test]
    fn windows_icon_contains_the_generated_vector_sizes() {
        let root = crate::root()
            .expect("runtime workspace")
            .join("assets/icons/line");
        let svg = std::fs::read_to_string(root.with_extension("svg")).expect("SVG source");
        assert_eq!(
            std::fs::read(root.with_extension("ico")).expect("LFS Windows icon"),
            windows_icon(&svg, &WINDOWS_ICON_SIZES, None).expect("multi-size Windows icon"),
            "Windows ICO differs from its SVG source; run cargo xtask icons"
        );
    }

    #[test]
    fn windows_tray_icon_matches_the_shared_status_artwork() {
        let root = crate::root()
            .expect("runtime workspace")
            .join("assets/icons");
        let svg = std::fs::read_to_string(root.join("status.svg")).expect("shared status SVG");
        assert_eq!(
            std::fs::read(root.join("status-windows.ico")).expect("LFS Windows tray icon"),
            windows_icon(&svg, &WINDOWS_TRAY_ICON_SIZES, Some(WINDOWS_TRAY_STYLE))
                .expect("multi-size Windows tray icon"),
            "Windows tray ICO differs from the shared status artwork; run cargo xtask icons"
        );
    }

    #[test]
    fn windows_tray_symbol_keeps_visible_ink_on_light_and_dark_surfaces() {
        let path = crate::root()
            .expect("runtime workspace")
            .join("assets/icons/status-windows.ico");
        let bytes = std::fs::read(path).expect("LFS Windows tray icon");
        let frames = ico_frames(&bytes);
        for frame in &frames {
            let size = frame.width();
            let mut ink = Vec::new();
            for (x, y, pixel) in frame.enumerate_pixels() {
                // Measure the decoded artifact after alpha compositing, not
                // just its nominal RGB or outer pale application-icon tile.
                if [[243, 243, 243], [48, 48, 48]]
                    .into_iter()
                    .all(|background| contrast_on(*pixel, background) >= 3.0)
                {
                    ink.push((x, y));
                }
            }
            // Project-specific readability guards, not OS-mandated occupancy:
            // substantial ink and a recognizable symbol span at small sizes.
            assert!(
                ink.len() as u32 >= size * size / 10,
                "too little readable ink at {size}px"
            );
            let left = ink.iter().map(|(x, _)| *x).min().expect("visible ink");
            let right = ink.iter().map(|(x, _)| *x).max().expect("visible ink");
            let top = ink.iter().map(|(_, y)| *y).min().expect("visible ink");
            let bottom = ink.iter().map(|(_, y)| *y).max().expect("visible ink");
            assert!(
                right - left + 1 >= size * 7 / 10,
                "narrow symbol at {size}px"
            );
            assert!(
                bottom - top + 1 >= size * 7 / 10,
                "short symbol at {size}px"
            );
            assert!(
                frame.enumerate_pixels().all(|(x, y, pixel)| {
                    (x != 0 && y != 0 && x != size - 1 && y != size - 1) || pixel[3] == 0
                }),
                "symbol must retain a transparent outer edge at {size}px"
            );
        }
        assert_eq!(
            frames
                .iter()
                .map(image::RgbaImage::width)
                .collect::<Vec<_>>(),
            [16, 20, 24, 28, 32, 40, 48, 64]
        );
    }

    fn contrast_on(pixel: image::Rgba<u8>, background: [u8; 3]) -> f64 {
        fn luminance(rgb: [f64; 3]) -> f64 {
            rgb.into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(channel, weight)| {
                    weight
                        * if channel <= 0.04045 {
                            channel / 12.92
                        } else {
                            ((channel + 0.055) / 1.055).powf(2.4)
                        }
                })
                .sum()
        }
        let background = background.map(|channel| f64::from(channel) / 255.0);
        let alpha = f64::from(pixel[3]) / 255.0;
        let composited = std::array::from_fn(|index| {
            f64::from(pixel[index]) / 255.0 * alpha + background[index] * (1.0 - alpha)
        });
        let foreground = luminance(composited);
        let background = luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    fn ico_frames(bytes: &[u8]) -> Vec<image::RgbaImage> {
        assert_eq!(&bytes[..4], &[0, 0, 1, 0], "ICO directory header");
        let count = u16::from_le_bytes(bytes[4..6].try_into().expect("ICO frame count"));
        bytes[6..6 + usize::from(count) * 16]
            .as_chunks::<16>()
            .0
            .iter()
            .map(|entry| {
                let length =
                    u32::from_le_bytes(entry[8..12].try_into().expect("frame length")) as usize;
                let start =
                    u32::from_le_bytes(entry[12..16].try_into().expect("frame offset")) as usize;
                let frame = image::load_from_memory(&bytes[start..start + length])
                    .expect("embedded PNG frame")
                    .to_rgba8();
                let dimension = |value| if value == 0 { 256 } else { u32::from(value) };
                assert_eq!(
                    frame.dimensions(),
                    (dimension(entry[0]), dimension(entry[1]))
                );
                frame
            })
            .collect()
    }
}
