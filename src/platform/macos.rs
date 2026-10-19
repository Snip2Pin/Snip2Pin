//! AppKit presentation without replacing backend classes or delegates.
use objc2_app_kit::{NSApplication, NSWindow};
use snip2pin::i18n::{Message, Text};

/// Explicit Home requests must activate the application even if its window
/// is already key: key-window status alone does not imply foreground activation.
pub fn activate_application() -> Result<(), Message> {
    let thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    let app = NSApplication::sharedApplication(thread);
    app.unhideWithoutActivation();
    // Supports the minimum macOS 13 target, matching winit's activation API.
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    Ok(())
}

pub fn prepare_pin(window: &NSWindow) -> Result<(), Message> {
    let _thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    let level = objc2_core_graphics::kCGFloatingWindowLevel as isize;
    if window.level() != level {
        window.setLevel(level);
    }
    Ok(())
}
