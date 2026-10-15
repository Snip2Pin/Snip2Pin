#[cfg(target_os = "macos")]
#[path = "../src/platform.rs"]
#[allow(dead_code)]
mod platform;

#[cfg(target_os = "macos")]
fn main() {
    use winit::{
        application::ApplicationHandler,
        dpi::{LogicalPosition, LogicalSize},
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS},
        window::{Theme, Window, WindowId, WindowLevel},
    };

    struct Lifecycle;
    impl ApplicationHandler for Lifecycle {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let image = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Snip2Pin · 贴图 42")
                        .with_decorations(false)
                        .with_resizable(true)
                        .with_transparent(false)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_inner_size(LogicalSize::new(300.0, 200.0))
                        .with_min_inner_size(LogicalSize::new(80.0, 50.0))
                        .with_visible(false),
                )
                .expect("hidden resizable image window");
            platform::prepare_floating_windows().expect("prepare native windows");
            let thread = objc2::MainThreadMarker::new().expect("main thread");
            let app = objc2_app_kit::NSApplication::sharedApplication(thread);
            let windows = app.windows();
            let native = windows
                .iter()
                .find(|window| window.title().to_string() == "Snip2Pin · 贴图 42")
                .expect("image");
            assert_eq!(
                windows
                    .iter()
                    .filter(|window| window.title().to_string().starts_with("Snip2Pin ·"))
                    .count(),
                1,
                "a pin has one native image window and no auxiliary controls window"
            );
            let class = native.class();
            assert!(
                native
                    .styleMask()
                    .contains(objc2_app_kit::NSWindowStyleMask::Resizable)
            );
            assert!(image.is_resizable());
            assert!(native.parentWindow().is_none());
            assert_eq!(native.contentMinSize().width, 80.0);
            assert_eq!(native.contentMinSize().height, 50.0);
            assert_eq!(
                native.level(),
                objc2_core_graphics::kCGFloatingWindowLevel as isize
            );
            for (width, height) in [(350.0, 230.0), (250.0, 180.0), (80.0, 50.0)] {
                let _ = image.request_inner_size(LogicalSize::new(width, height));
                let actual = native.contentRectForFrameRect(native.frame());
                assert_eq!(actual.size.width, width);
                assert_eq!(actual.size.height, height);
                assert_eq!(
                    actual,
                    native.frame(),
                    "borderless image content has no toolbar offset"
                );
                image.set_outer_position(LogicalPosition::new(100.0, 180.0));
                for theme in [Some(Theme::Dark), Some(Theme::Light), None] {
                    image.set_theme(theme);
                }
                platform::prepare_floating_windows().expect("repeat style preparation");
                assert_eq!(
                    native.class(),
                    class,
                    "do not replace observed native classes"
                );
                assert!(native.parentWindow().is_none());
                assert_eq!(native.contentMinSize().width, 80.0);
                assert_eq!(native.contentMinSize().height, 50.0);
                assert_eq!(
                    native.contentRectForFrameRect(native.frame()),
                    native.frame()
                );
                assert!(!native.isVisible());
                assert!(!native.isKeyWindow());
                assert!(!app.isActive());
            }
            drop(windows);
            drop(image);
            event_loop.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Prohibited)
        .with_activate_ignoring_other_apps(false)
        .build()
        .expect("hidden event loop");
    event_loop
        .run_app(&mut Lifecycle)
        .expect("native lifecycle");
    println!("macOS hidden native image lifecycle passed; visible live-resize remains unverified");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
