/// Render the shared vector artwork for both native windows and app packages.
pub fn render(size: u32) -> Result<image::RgbaImage, String> {
    let tree = resvg::usvg::Tree::from_str(
        include_str!("../assets/icon.svg"),
        &resvg::usvg::Options::default(),
    )
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
