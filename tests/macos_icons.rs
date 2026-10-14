#[cfg(target_os = "macos")]
#[path = "../src/platform.rs"]
#[allow(dead_code)]
mod platform;

#[cfg(target_os = "macos")]
fn main() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use snip2pin::icon::Variant;

    let thread = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(thread);
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let mut previous = None;
    let mut line = None;
    for variant in [Variant::Line, Variant::Metal, Variant::Line] {
        let ctx = eframe::egui::Context::default();
        let pixels = snip2pin::icon::render(variant, snip2pin::icon::WINDOW_SIZE).expect("pixels");
        platform::set_app_icon(
            &ctx,
            variant,
            std::sync::Arc::new(eframe::egui::IconData {
                width: pixels.width(),
                height: pixels.height(),
                rgba: pixels.into_raw(),
            }),
        )
        .expect("select icon");
        let dock = app
            .applicationIconImage()
            .expect("Dock icon")
            .TIFFRepresentation()
            .expect("icon pixels")
            .to_vec();
        if let Some(previous) = previous.replace(dock.clone()) {
            assert!(dock != previous, "Dock must reflect each icon selection");
        }
        if variant == Variant::Line
            && let Some(first) = line.replace(dock.clone())
        {
            assert!(dock == first, "switching back restores the original icon");
        }
        assert!(!app.isActive());
        assert!(app.windows().is_empty());
    }
    println!("macOS Dock icon: line/metal/line switches verified without activation");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
