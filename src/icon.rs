#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Line,
    Metal,
}

impl Variant {
    fn svg(self) -> &'static str {
        match self {
            Self::Line => include_str!("../assets/icons/line.svg"),
            Self::Metal => include_str!("../assets/icons/metal.svg"),
        }
    }

    fn png(self) -> &'static [u8] {
        match self {
            Self::Line => include_bytes!("../assets/icons/line.png"),
            Self::Metal => include_bytes!("../assets/icons/metal.png"),
        }
    }
}

/// Load the LFS-managed PNG used by the application.
pub fn render(variant: Variant, size: u32) -> Result<image::RgbaImage, String> {
    if size == 0 {
        return Err("Invalid icon size".into());
    }
    let image = image::load_from_memory(variant.png())
        .map_err(|error| format!("图标资源无效，请先运行 git lfs pull：{error}"))?
        .to_rgba8();
    if image.width() == size && image.height() == size {
        return Ok(image);
    }
    Ok(image::imageops::resize(
        &image,
        size,
        size,
        image::imageops::FilterType::Lanczos3,
    ))
}

/// Regenerate PNG assets from their editable SVG sources.
pub fn render_vector(variant: Variant, size: u32) -> Result<image::RgbaImage, String> {
    let tree = resvg::usvg::Tree::from_str(variant.svg(), &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size, size).ok_or_else(|| "Invalid icon size".to_owned())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(size as f32 / 1024.0, size as f32 / 1024.0),
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
    image::RgbaImage::from_raw(size, size, rgba).ok_or_else(|| "Invalid icon pixels".to_owned())
}
