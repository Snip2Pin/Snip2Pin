//! Floating level only; AppKit owns movement, resizing and presentation.
use objc2_app_kit::NSWindow;

pub fn prepare_pin(window: &NSWindow) -> Result<(), String> {
    let _thread = objc2::MainThreadMarker::new().ok_or("贴图窗口必须在 UI 线程准备。")?;
    let level = objc2_core_graphics::kCGFloatingWindowLevel as isize;
    if window.level() != level {
        window.setLevel(level);
    }
    Ok(())
}
