//! macOS Dock icons use owned pixels; bundle icons are selected before signing.
use snip2pin::i18n::{Message, Text};
#[allow(unsafe_code)]
pub fn update(variant: snip2pin::icon::Variant) -> Result<(), Message> {
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSApplication, NSBitmapFormat, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage,
    };
    use objc2_foundation::NSSize;
    let main_thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    const RGBA_CHANNELS: usize = 4;
    const BITS_PER_CHANNEL: isize = u8::BITS as isize;
    let pixels = snip2pin::icon::render(variant, snip2pin::icon::DOCK_SIZE)?;
    let row_len = pixels.width() as usize * RGBA_CHANNELS;
    // SAFETY: Main-thread operation, bounded RGBA pixels, valid color
    // space and matching layout. Null planes request memory owned by AppKit.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), pixels.width() as isize, pixels.height() as isize,
            BITS_PER_CHANNEL, RGBA_CHANNELS as isize, true, false,
            NSDeviceRGBColorSpace, NSBitmapFormat::AlphaNonpremultiplied, row_len as isize, BITS_PER_CHANNEL * RGBA_CHANNELS as isize,
        )
    }.ok_or(Text::AllocateIcon)?;
    let stride = usize::try_from(bitmap.bytesPerRow()).map_err(|_| Text::IconStride)?;
    let destination = bitmap.bitmapData();
    if destination.is_null() || stride < row_len {
        return Err(Text::IconBuffer.into());
    }
    for (y, row) in pixels.as_raw().chunks_exact(row_len).enumerate() {
        // SAFETY: The validated native stride covers every RGBA row. The bitmap
        // owns all rows; Rust's source buffer is distinct and still alive.
        unsafe {
            std::ptr::copy_nonoverlapping(row.as_ptr(), destination.add(y * stride), row_len);
        }
    }
    let image = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize::new(f64::from(pixels.width()), f64::from(pixels.height())),
    );
    image.addRepresentation(&bitmap);
    // SAFETY: The main-thread application receives Some(valid retained image),
    // never nil. AppKit retains the image and its owned bitmap representation.
    unsafe {
        NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&image));
    }
    Ok(())
}

/// Packaging reads saved preferences without showing or activating an app.
pub fn selected_variant() -> Result<snip2pin::icon::Variant, Message> {
    use objc2_foundation::{NSString, NSUserDefaults};
    let settings = snip2pin::settings::path().ok_or(Text::NoConfigDirectory)?;
    let preferences = snip2pin::settings::Preferences::load(&settings)?;
    let style = NSUserDefaults::standardUserDefaults()
        .stringForKey(&NSString::from_str("AppleInterfaceStyle"));
    let theme = if style.is_some_and(|style| style.to_string() == "Dark") {
        eframe::egui::Theme::Dark
    } else {
        eframe::egui::Theme::Light
    };
    Ok(preferences.icon_variant(Some(theme)))
}
