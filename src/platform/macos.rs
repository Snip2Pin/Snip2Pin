//! AppKit presentation without replacing backend classes or delegates.
use objc2_app_kit::{NSApplication, NSScreen, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use snip2pin::i18n::{Message, Text};

/// The primary screen's top defines the backend's native-point desktop origin.
pub fn desktop_top() -> Option<f64> {
    let thread = objc2::MainThreadMarker::new()?;
    let primary = NSScreen::screens(thread).firstObject()?;
    Some(primary_top(primary.frame()))
}

pub fn primary_top(frame: NSRect) -> f64 {
    frame.origin.y + frame.size.height
}

pub fn backend_rect(rect: NSRect, top: f64) -> eframe::egui::Rect {
    eframe::egui::Rect::from_min_size(
        eframe::egui::pos2(
            rect.origin.x as f32,
            (top - rect.origin.y - rect.size.height) as f32,
        ),
        eframe::egui::vec2(rect.size.width as f32, rect.size.height as f32),
    )
}

pub fn native_rect(rect: eframe::egui::Rect, top: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(f64::from(rect.min.x), top - f64::from(rect.max.y)),
        NSSize::new(f64::from(rect.width()), f64::from(rect.height())),
    )
}

/// Current usable bounds in the backend's top-left desktop coordinates.
pub fn work_area(screen: &NSScreen) -> Option<eframe::egui::Rect> {
    Some(backend_rect(screen.visibleFrame(), desktop_top()?))
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

#[cfg(test)]
mod coordinate_tests {
    #[test]
    fn backend_coordinates_round_trip_negative_and_above_primary_screens() {
        use super::*;
        let top = primary_top(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(1920.0, 1080.0),
        ));
        for (native, expected) in [
            (
                NSRect::new(NSPoint::new(-1280.0, 0.0), NSSize::new(1280.0, 1024.0)),
                eframe::egui::Rect::from_min_size(
                    eframe::egui::pos2(-1280.0, 56.0),
                    eframe::egui::vec2(1280.0, 1024.0),
                ),
            ),
            (
                NSRect::new(NSPoint::new(0.0, 1080.0), NSSize::new(1600.0, 900.0)),
                eframe::egui::Rect::from_min_size(
                    eframe::egui::pos2(0.0, -900.0),
                    eframe::egui::vec2(1600.0, 900.0),
                ),
            ),
        ] {
            assert_eq!(backend_rect(native, top), expected);
            assert_eq!(native_rect(expected, top), native);
        }
    }
}
