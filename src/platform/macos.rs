//! AppKit presentation without replacing backend classes or delegates.
use objc2_app_kit::{NSApplication, NSWindow};
use objc2_foundation::NSSize;
use snip2pin::i18n::{Message, Text};

/// Original native constraints, restored exactly when a pin is unlocked.
pub struct PinLock {
    min: NSSize,
    max: NSSize,
    movable: bool,
}

pub fn set_locked(window: &NSWindow, locked: bool, saved: &mut Option<PinLock>) {
    if locked && saved.is_none() {
        *saved = Some(PinLock {
            min: window.contentMinSize(),
            max: window.contentMaxSize(),
            movable: window.isMovable(),
        });
        let size = window.contentRectForFrameRect(window.frame()).size;
        // Keep the backend's resizable style unchanged. Changing style masks on
        // an ordered, spanning window needlessly reconfigures its presentation.
        window.setContentMinSize(size);
        window.setContentMaxSize(size);
        window.setMovable(false);
    } else if !locked && let Some(original) = saved.take() {
        window.setContentMaxSize(original.max);
        window.setContentMinSize(original.min);
        window.setMovable(original.movable);
    }
}

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
