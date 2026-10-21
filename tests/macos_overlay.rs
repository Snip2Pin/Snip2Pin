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
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

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
    let primary_frame = screens.iter().next().expect("primary screen").frame();
    let desktop_top = primary_frame.origin.y + primary_frame.size.height;
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
        window.setTitle(&NSString::from_str(snip2pin::SELECTION_WINDOW_TITLE));
        platform::prepare_floating_windows().expect("prepare native windows");
        assert!(!window.isVisible());
        assert!(!application.isActive());
        let actual = window.frame();
        assert_eq!(
            actual, expected,
            "selection layer must include the menu bar and Dock, without compressing the captured desktop"
        );
        assert!(window.level() > objc2_core_graphics::kCGStatusWindowLevel as isize);
        platform::prepare_floating_windows().expect("prepare native windows");
        assert_eq!(window.frame(), expected, "preparation must be idempotent");
        assert!(!window.isVisible());
        assert!(!application.isActive());

        let scale = screen.backingScaleFactor();
        let display = snip2pin::capture::Display {
            id: 0,
            origin: [
                expected.origin.x as f32,
                (desktop_top - expected.origin.y - expected.size.height) as f32,
            ],
            size: [expected.size.width as f32, expected.size.height as f32],
            scale: scale as f32,
        };
        let visible = screen.visibleFrame();
        let usable = platform::pin_work_areas(std::slice::from_ref(&display))[0];
        assert_eq!(
            usable.min,
            eframe::egui::pos2(
                visible.origin.x as f32,
                (desktop_top - visible.origin.y - visible.size.height) as f32
            )
        );
        assert_eq!(
            usable.size(),
            eframe::egui::vec2(visible.size.width as f32, visible.size.height as f32)
        );
        let target = NSRect::new(
            NSPoint::new(expected.origin.x + 100.0, expected.origin.y + 100.0),
            NSSize::new(301.0 / scale, 201.0 / scale),
        );
        // SAFETY: Valid borderless geometry on the main thread. This window is
        // never shown; the same native APIs are used by winit's size/move commands.
        let pin = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(thread),
                target,
                NSWindowStyleMask::Borderless | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                true,
            )
        };
        let outside = NSRect::new(
            NSPoint::new(
                target.origin.x,
                (visible.origin.y + visible.size.height - target.size.height / 2.0).floor(),
            ),
            target.size,
        );
        pin.setLevel(objc2_core_graphics::kCGFloatingWindowLevel as isize);
        assert_ne!(
            pin.constrainFrameRect_toScreen(outside, Some(&screen)),
            outside
        );
        let original_class = pin.class();
        pin.setTitle(&NSString::from_str("Snip2Pin · 贴图"));
        platform::prepare_floating_windows().expect("prepare native windows");
        // AppKit owns the accepted native content size and position.
        let window_size = NSSize::new(target.size.width.ceil(), target.size.height.ceil());
        pin.setContentSize(window_size);
        pin.setFrameOrigin(target.origin);
        assert_eq!(
            pin.frame(),
            NSRect::new(target.origin, window_size),
            "native rounding must be accounted for before positioning the pin"
        );
        let image_top = visible.origin.y + visible.size.height;
        let origin = NSPoint::new(target.origin.x, image_top - window_size.height);
        pin.setFrameOrigin(origin);
        assert_eq!(
            pin.frame().origin,
            origin,
            "image content can touch the menu bar without transparent margins"
        );
        assert_eq!(pin.frame().origin.y + pin.frame().size.height, image_top);
        assert_eq!(
            pin.frame().size,
            window_size,
            "top-edge movement cannot shrink the image"
        );
        let offscreen = NSRect::new(
            NSPoint::new(
                target.origin.x,
                (image_top + window_size.height / 2.0 - window_size.height).floor(),
            ),
            window_size,
        );
        platform::prepare_floating_windows().expect("prepare native windows");
        assert_eq!(
            pin.level(),
            objc2_core_graphics::kCGFloatingWindowLevel as isize
        );
        assert!(pin.level() < objc2_core_graphics::kCGDockWindowLevel as isize);
        assert!(pin.level() < objc2_core_graphics::kCGMainMenuWindowLevel as isize);
        assert_eq!(
            pin.class(),
            original_class,
            "preserve AppKit observation bookkeeping"
        );
        assert!(
            pin.level() < window.level(),
            "capture layers remain above existing pins"
        );
        pin.setFrameOrigin(offscreen.origin);
        platform::prepare_floating_windows().expect("prepare native windows");
        assert_eq!(
            pin.frame(),
            offscreen,
            "preparation must not snap an offscreen pin back into place"
        );
        assert!(!pin.isVisible());
        assert!(!application.isActive());
    }
    let pointer =
        platform::pointer_position().expect("cursor position is readable without event hooks");
    let displays = snip2pin::capture::displays().expect("display bounds");
    if displays.is_empty() {
        println!("macOS cursor/monitor check unavailable: no displays exposed by the session");
        println!("macOS hidden selection geometry: full display bounds preserved");
        return;
    }
    assert!(
        displays.iter().any(|display| display.contains(pointer)),
        "cursor {pointer:?} and monitors {displays:?} must use the same space"
    );
    println!("macOS hidden selection geometry: full display bounds preserved");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
