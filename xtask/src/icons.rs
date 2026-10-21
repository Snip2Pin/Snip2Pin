//! Generate runtime PNGs without linking SVG rendering into the application.
use crate::Result;

const ICONS: [(&str, u32); 3] = [("line", 1024), ("metal", 1024), ("status", 32)];

pub fn generate() -> Result {
    for (name, size) in ICONS {
        render(
            &std::fs::read_to_string(format!("assets/icons/{name}.svg"))?,
            size,
        )?
        .save(format!("assets/icons/{name}.png"))?;
    }
    Ok(())
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
}
