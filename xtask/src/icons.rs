//! Generate native icon assets without linking SVG rendering into the application.
use crate::Result;

const ICONS: [(&str, u32); 3] = [("line", 1024), ("metal", 1024), ("status", 32)];
const WINDOWS_ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

pub fn generate() -> Result {
    for (name, size) in ICONS {
        render(
            &std::fs::read_to_string(format!("assets/icons/{name}.svg"))?,
            size,
        )?
        .save(format!("assets/icons/{name}.png"))?;
    }
    let svg = std::fs::read_to_string("assets/icons/line.svg")?;
    std::fs::write("assets/icons/line.ico", windows_icon(&svg)?)?;
    Ok(())
}

fn windows_icon(svg: &str) -> Result<Vec<u8>> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};

    let frames = WINDOWS_ICON_SIZES
        .into_iter()
        .map(|size| {
            let pixels = render(svg, size)?;
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

fn render(svg: &str, size: u32) -> Result<image::RgbaImage> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())?;
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
        let root = crate::root();
        for (name, size) in ICONS {
            let stem = root.join("assets/icons").join(name);
            let svg = std::fs::read_to_string(stem.with_extension("svg")).expect("SVG source");
            let png = image::open(stem.with_extension("png"))
                .expect("LFS PNG")
                .to_rgba8();
            assert_eq!(png.dimensions(), (size, size), "{name} dimensions");
            assert!(
                png == render(&svg, size).expect("SVG rasterization"),
                "{name} PNG differs from its SVG source; run cargo xtask icons"
            );
        }
    }

    #[test]
    fn windows_icon_contains_the_generated_vector_sizes() {
        let root = crate::root().join("assets/icons/line");
        let svg = std::fs::read_to_string(root.with_extension("svg")).expect("SVG source");
        assert_eq!(
            std::fs::read(root.with_extension("ico")).expect("LFS Windows icon"),
            windows_icon(&svg).expect("multi-size Windows icon"),
            "Windows ICO differs from its SVG source; run cargo xtask icons"
        );
    }
}
