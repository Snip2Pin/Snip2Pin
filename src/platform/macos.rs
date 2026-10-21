//! AppKit presentation without replacing backend classes or delegates.
use objc2_app_kit::{NSApplication, NSScreen, NSWindow};
use objc2_foundation::NSSize;
use snip2pin::i18n::{Message, Text};

/// Current usable bounds in the backend's top-left desktop coordinates.
pub fn work_area(screen: &NSScreen) -> Option<eframe::egui::Rect> {
    let thread = objc2::MainThreadMarker::new()?;
    let primary = NSScreen::screens(thread).firstObject()?;
    let desktop_top = primary.frame().origin.y + primary.frame().size.height;
    let visible = screen.visibleFrame();
    Some(eframe::egui::Rect::from_min_size(
        eframe::egui::pos2(
            visible.origin.x as f32,
            (desktop_top - visible.origin.y - visible.size.height) as f32,
        ),
        eframe::egui::vec2(visible.size.width as f32, visible.size.height as f32),
    ))
}

pub fn pin_presentation_area(id: u64) -> Option<eframe::egui::Rect> {
    let thread = objc2::MainThreadMarker::new()?;
    if !NSScreen::screensHaveSeparateSpaces(thread) {
        return None;
    }
    let window = super::pin_window::find(id).ok()?;
    let screen = window.screen()?;
    work_area(&screen)
}

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
