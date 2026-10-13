#[cfg(target_os = "macos")]
#[path = "../src/platform.rs"]
#[allow(dead_code)]
mod platform;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn main() {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSScreen, NSWindow,
        NSWindowStyleMask,
    };
    use objc2_foundation::NSString;

    let thread = MainThreadMarker::new().expect("native geometry test runs on the main thread");
    let application = NSApplication::sharedApplication(thread);
    application.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    assert_eq!(
        application.activationPolicy(),
        NSApplicationActivationPolicy::Prohibited
    );
    let screens = NSScreen::screens(thread);
    assert!(
        !screens.is_empty(),
        "macOS geometry test requires a display"
    );
    for screen in screens {
        let expected = screen.frame();
        // Reproduce a selection layer constrained to the desktop work area.
        // The window is never shown or activated, and no desktop pixels are read.
        // SAFETY: Valid screen rectangle, standard borderless style/backing, main
        // thread. The retained window remains hidden for its entire lifetime.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(thread),
                screen.visibleFrame(),
                NSWindowStyleMask::Borderless | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                true,
            )
        };
        window.setTitle(&NSString::from_str("SnipSprig · 框选"));
        platform::prepare_floating_windows();
        assert!(!window.isVisible());
        assert!(!application.isActive());
        let actual = window.frame();
        assert_eq!(
            actual, expected,
            "selection layer must include the menu bar and Dock, without compressing the captured desktop"
        );
        assert!(window.level() > objc2_core_graphics::kCGStatusWindowLevel as isize);
        platform::prepare_floating_windows();
        assert_eq!(window.frame(), expected, "preparation must be idempotent");
        assert!(!window.isVisible());
        assert!(!application.isActive());
    }
    println!("macOS hidden selection geometry: full display bounds preserved");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
