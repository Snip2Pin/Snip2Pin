use crate::i18n::{Message, Text};

pub const WINDOW_SIZE: u32 = 64;
pub const DOCK_SIZE: u32 = 256;
pub const EXPORT_SIZE: u32 = 1024;
pub const STATUS_SIZE: u32 = 32;

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
pub fn render(variant: Variant, size: u32) -> Result<image::RgbaImage, Message> {
    if size == 0 {
        return Err("Invalid icon size".into());
    }
    let image = image::load_from_memory(variant.png())
        .map_err(|error| Text::InvalidIcon.with(error))?
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
pub fn render_vector(variant: Variant, size: u32) -> Result<image::RgbaImage, Message> {
    render_svg(variant.svg(), size)
}

pub fn status() -> Result<image::RgbaImage, Message> {
    render_svg(include_str!("../assets/icons/status.svg"), STATUS_SIZE)
}

fn render_svg(svg: &str, size: u32) -> Result<image::RgbaImage, Message> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size, size).ok_or_else(|| "Invalid icon size".to_owned())?;
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
