//! Pin windows remain below system UI without changing their runtime class.
use objc2_app_kit::NSWindow;

pub fn prepare_pin(window: &NSWindow) -> Result<(), String> {
    let _thread = objc2::MainThreadMarker::new().ok_or("贴图窗口必须在 UI 线程准备。")?;
    let level = objc2_core_graphics::kCGFloatingWindowLevel as isize;
    if window.level() != level {
        window.setLevel(level);
    }
    Ok(())
}

pub fn layout_pin(window: &NSWindow, frame: snip2pin::pin::Frame) -> Option<eframe::egui::Rect> {
    let thread = objc2::MainThreadMarker::new()?;
    let primary = objc2_app_kit::NSScreen::screens(thread)
        .iter()
        .next()?
        .frame();
    let top = primary.origin.y + primary.size.height;
    let target = objc2_foundation::NSRect::new(
        objc2_foundation::NSPoint::new(
            f64::from(frame.window.min.x * frame.grid),
            top - f64::from(frame.window.max.y * frame.grid),
        ),
        objc2_foundation::NSSize::new(
            f64::from(frame.window.width() * frame.grid),
            f64::from(frame.window.height() * frame.grid),
        ),
    );
    if window.frame() != target {
        window.setFrame_display(target, false);
    }
    let actual = window.contentRectForFrameRect(window.frame());
    Some(eframe::egui::Rect::from_min_size(
        eframe::egui::pos2(
            actual.origin.x as f32 / frame.grid,
            (top - actual.origin.y - actual.size.height) as f32 / frame.grid,
        ),
        eframe::egui::vec2(
            actual.size.width as f32 / frame.grid,
            actual.size.height as f32 / frame.grid,
        ),
    ))
}
