//! Floating level only; AppKit owns movement, resizing and presentation.
use objc2_app_kit::NSWindow;
use snip2pin::i18n::{Message, Text};

pub fn prepare_pin(window: &NSWindow) -> Result<(), Message> {
    let _thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    let level = objc2_core_graphics::kCGFloatingWindowLevel as isize;
    if window.level() != level {
        window.setLevel(level);
    }
    Ok(())
}
