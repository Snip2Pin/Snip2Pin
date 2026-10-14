//! Only pin windows opt out of AppKit's frame constraint; their stacking level
//! stays below the Dock and menu bar. All inherited window behavior is retained.
use std::ffi::CString;

use objc2::{
    runtime::{AnyClass, AnyObject, ClassBuilder, Sel},
    sel,
};
use objc2_app_kit::{NSScreen, NSWindow};
use objc2_foundation::NSRect;

extern "C" fn unconstrained_frame(
    _window: &AnyObject,
    _selector: Sel,
    frame: NSRect,
    _screen: Option<&NSScreen>,
) -> NSRect {
    frame
}

#[allow(unsafe_code)]
pub fn prepare_pin(window: &NSWindow) -> Result<(), String> {
    let _thread = objc2::MainThreadMarker::new().ok_or("贴图窗口必须在 UI 线程准备。")?;
    let object: &AnyObject = window;
    let parent = object.class();
    const CLASS_PREFIX: &str = "Snip2PinFreeFrame_";
    if !parent
        .name()
        .to_bytes()
        .starts_with(CLASS_PREFIX.as_bytes())
    {
        let name = CString::new(format!("{CLASS_PREFIX}{}", parent.name().to_string_lossy()))
            .map_err(|_| "无效原生窗口类型。")?;
        let class = if let Some(mut builder) = ClassBuilder::new(&name, parent) {
            // SAFETY: Exactly NSWindow's documented method signature. No ivars,
            // ownership hooks or event methods are added or changed.
            unsafe {
                builder.add_method(
                    sel!(constrainFrameRect:toScreen:),
                    unconstrained_frame as extern "C" fn(_, _, _, _) -> _,
                );
            }
            builder.register()
        } else {
            AnyClass::get(&name).ok_or("无法准备贴图窗口类型。")?
        };
        if class.superclass() != Some(parent) || class.instance_size() != parent.instance_size() {
            return Err("贴图窗口类型不兼容。".into());
        }
        // SAFETY: Main thread, this app's pin only. The checked class directly
        // subclasses the actual native class (including winit's ivars), adds no
        // storage, and overrides only a compatible pure geometry method. The
        // original class and every other window's methods remain untouched.
        let previous = unsafe { AnyObject::set_class(object, class) };
        assert_eq!(previous, parent, "native window class changed concurrently");
    }
    let level = objc2_core_graphics::kCGFloatingWindowLevel as isize;
    if window.level() != level {
        window.setLevel(level);
    }
    Ok(())
}
