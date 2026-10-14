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
            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Snip2Pin · 贴图")
                        .with_decorations(false)
                        .with_resizable(false)
                        .with_transparent(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_inner_size(LogicalSize::new(300.0, 200.0))
                        .with_visible(false),
                )
                .expect("create hidden winit pin with appearance observation");
            platform::prepare_floating_windows().expect("prepare real backend pin");
            let thread = objc2::MainThreadMarker::new().expect("main thread");
            let app = objc2_app_kit::NSApplication::sharedApplication(thread);
            let mut tested = false;
            for native in app.windows() {
                if native.title().to_string() != "Snip2Pin · 贴图" {
                    continue;
                }
                let screen = native.screen().expect("pin screen");
                let bounds = screen.frame();
                let original_class = native.class();
                let mut target = native.frame();
                target.origin.y =
                    (bounds.origin.y + bounds.size.height - target.size.height / 2.0).floor();
                let primary = objc2_app_kit::NSScreen::screens(thread)
                    .iter()
                    .next()
                    .expect("primary screen")
                    .frame();
                let desktop_top = primary.origin.y + primary.size.height;
                let visible = screen.visibleFrame();
                let usable = eframe::egui::Rect::from_min_size(
                    eframe::egui::pos2(
                        visible.origin.x as f32,
                        (desktop_top - visible.origin.y - visible.size.height) as f32,
                    ),
                    eframe::egui::vec2(visible.size.width as f32, visible.size.height as f32),
                );
                let frame = snip2pin::pin::frame_geometry(
                    snip2pin::geometry::Placement {
                        position: [target.origin.x as f32, usable.min.y + 100.0],
                        size: [target.size.width as f32, target.size.height as f32],
                    },
                    usable,
                    1.0,
                    1.0,
                );
                let mut moved = frame.moved_to(
                    eframe::egui::pos2(
                        target.origin.x as f32,
                        usable.min.y - frame.image.height() / 2.0,
                    ),
                    &[usable],
                    1.0,
                );
                moved.fit_host_top();
                target = objc2_foundation::NSRect::new(
                    objc2_foundation::NSPoint::new(
                        f64::from(moved.window.min.x),
                        desktop_top - f64::from(moved.window.max.y),
                    ),
                    objc2_foundation::NSSize::new(
                        f64::from(moved.window.width()),
                        f64::from(moved.window.height()),
                    ),
                );
                assert_eq!(
                    native.constrainFrameRect_toScreen(target, Some(&screen)),
                    target,
                    "native host must not be moved by the menu-bar constraint"
                );
                assert_eq!(
                    moved.image.size(),
                    frame.image.size(),
                    "original image pixels are not scaled"
                );
                assert_eq!(moved.image.min.y, 0.0, "picture touches the menu bar");
                assert!(
                    eframe::egui::Rect::from_min_size(
                        eframe::egui::Pos2::ZERO,
                        moved.window.size()
                    )
                    .contains_rect(moved.image),
                    "complete image stays inside the native host"
                );
                for button in [moved.toolbar_rects().0, moved.toolbar_rects().1] {
                    assert!(usable.contains_rect(button.translate(moved.window.min.to_vec2())));
                }
                let _ = window
                    .request_inner_size(LogicalSize::new(target.size.width, target.size.height));
                window.set_outer_position(LogicalPosition::new(
                    target.origin.x,
                    desktop_top - target.origin.y - target.size.height,
                ));
                assert_eq!(
                    native.frame().origin,
                    target.origin,
                    "image-aware native host is accepted without a position correction"
                );
                window.set_theme(Some(Theme::Dark));
                window.set_theme(Some(Theme::Light));
                window.set_theme(None);
                platform::prepare_floating_windows().expect("repeat preparation");
                assert_eq!(
                    native.class(),
                    original_class,
                    "preserve observed native type"
                );
                assert_eq!(
                    native.frame(),
                    target,
                    "appearance changes preserve original size and position"
                );
                assert!(!native.isVisible());
                assert!(!app.isActive());
                tested = true;
            }
            assert!(tested, "exercise the real native backend window");
            assert!(!window.is_visible().unwrap_or(true));
            drop(window);
            event_loop.exit();
        }

        fn window_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            _event: WindowEvent,
        ) {
        }
    }

    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Prohibited)
        .with_activate_ignoring_other_apps(false)
        .build()
        .expect("main-thread hidden native test");
    event_loop.run_app(&mut Lifecycle).expect("destroy pin");
    println!("macOS winit pin lifecycle: appearance observer removed without crashing");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
