use crate::i18n::{Message, Text};

pub const WINDOW_SIZE: u32 = 64;
pub const DOCK_SIZE: u32 = 256;
pub const STATUS_SIZE: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Line,
    Metal,
}

impl Variant {
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

pub fn status() -> Result<image::RgbaImage, Message> {
    image::load_from_memory(include_bytes!("../assets/icons/status.png"))
        .map(|image| image.to_rgba8())
        .map_err(|error| Text::InvalidIcon.with(error))
}
