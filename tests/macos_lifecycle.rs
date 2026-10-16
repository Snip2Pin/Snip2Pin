#[cfg(target_os = "macos")]
#[path = "../src/platform.rs"]
#[allow(dead_code)]
mod platform;

#[cfg(target_os = "macos")]
fn main() {
    use eframe::egui;
    use snip2pin::{capture, geometry, pin};
    use std::sync::Arc;
    use winit::{
        application::ApplicationHandler,
        dpi::{LogicalPosition, LogicalSize},
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS},
        window::{Theme, Window, WindowId, WindowLevel},
    };

    struct TransparentOrder {
        window: objc2::rc::Retained<objc2_app_kit::NSWindow>,
        frame: objc2_foundation::NSRect,
        style: objc2_app_kit::NSWindowStyleMask,
    }
    impl TransparentOrder {
        fn assert_unchanged(&self, app: &objc2_app_kit::NSApplication) {
            assert_eq!(
                self.window.frame(),
                self.frame,
                "viewport info moved the pin"
            );
            assert_eq!(self.window.styleMask(), self.style);
            assert_eq!(
                self.window.contentRectForFrameRect(self.window.frame()),
                self.frame,
                "content origin and size must remain unchanged"
            );
            let view = self.window.contentView().expect("ordered pin content view");
            assert_eq!(
                self.window
                    .convertRectToScreen(view.convertRect_toView(view.bounds(), None)),
                self.frame,
                "content view must fill the pin"
            );
            assert_eq!(self.window.alphaValue(), 0.0);
            assert!(self.window.ignoresMouseEvents());
            assert!(!self.window.isKeyWindow() && !app.isActive());
        }
    }
    impl Drop for TransparentOrder {
        fn drop(&mut self) {
            self.window.close();
        }
    }

    #[derive(Default)]
    struct Lifecycle {
        transparent_orders: Vec<TransparentOrder>,
        top_windows: Vec<Window>,
    }
    impl ApplicationHandler for Lifecycle {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let drag = platform::macos_drag::DragEvents::install().expect("local drag monitor");
            drop(drag);
            let sizes = [(300.0, 200.0), (1.0, 1.0), (12.0, 120.0)];
            let images: Vec<_> = sizes
                .iter()
                .enumerate()
                .map(|(index, &(width, height))| {
                    event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title(format!("Snip2Pin · 贴图 {index}"))
                                .with_decorations(false)
                                .with_resizable(true)
                                .with_transparent(false)
                                .with_window_level(WindowLevel::AlwaysOnTop)
                                .with_inner_size(LogicalSize::new(width, height))
                                .with_visible(false),
                        )
                        .expect("hidden resizable image window")
                })
                .collect();
            platform::prepare_floating_windows().expect("prepare native windows");
            let thread = objc2::MainThreadMarker::new().expect("main thread");
            let app = objc2_app_kit::NSApplication::sharedApplication(thread);
            let windows = app.windows();
            assert_eq!(
                windows
                    .iter()
                    .filter(|window| window.title().to_string().starts_with("Snip2Pin ·"))
                    .count(),
                images.len(),
                "each pin has one native image window and no auxiliary controls window"
            );
            for (index, (image, (initial_width, initial_height))) in
                images.iter().zip(sizes).enumerate()
            {
                let title = format!("Snip2Pin · 贴图 {index}");
                let native = windows
                    .iter()
                    .find(|window| window.title().to_string() == title)
                    .expect("image");
                let class = native.class();
                assert!(
                    native
                        .styleMask()
                        .contains(objc2_app_kit::NSWindowStyleMask::Resizable)
                );
                assert!(image.is_resizable());
                assert!(native.parentWindow().is_none());
                let initial = native.contentRectForFrameRect(native.frame());
                #[cfg(test)]
                if index == 0 {
                    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
                    use objc2_foundation::NSPoint;
                    let view = native.contentView().expect("pin view");
                    let point = NSPoint::new(111.7, 74.1);
                    let window_point = view.convertPoint_toView(point, None);
                    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                        NSEventType::LeftMouseDown, window_point, NSEventModifierFlags::empty(),
                        123.0, native.windowNumber(), None, 42, 1, 1.0,
                    ).expect("synthetic press; never posted to AppKit");
                    let drag = platform::macos_drag::DragEvents::default();
                    drag.record_native_for_test(event);
                    let physical = [
                        point.x * native.backingScaleFactor(),
                        point.y * native.backingScaleFactor(),
                    ];
                    for zoom in [1.0, 1.25, 2.0] {
                        let pixels_per_point = image.scale_factor() as f32 * zoom;
                        let origin = egui::pos2(
                            physical[0] as f32 / pixels_per_point,
                            physical[1] as f32 / pixels_per_point,
                        );
                        let requests = drag.requests();
                        assert!(
                            requests
                                .matching(index as u64, origin, pixels_per_point)
                                .is_some()
                        );
                        assert!(
                            requests
                                .matching(index as u64 + 1, origin, pixels_per_point)
                                .is_none()
                        );
                        assert!(
                            requests
                                .matching(
                                    index as u64,
                                    origin + egui::vec2(1.0, 0.0),
                                    pixels_per_point
                                )
                                .is_none()
                        );
                    }
                    println!(
                        "macOS synthetic original-press mapping passed without event delivery"
                    );
                }
                assert_eq!(initial.size.width, initial_width);
                assert_eq!(initial.size.height, initial_height);
                assert!(native.contentMinSize().width <= 1.0);
                assert!(native.contentMinSize().height <= 1.0);
                assert_eq!(
                    native.level(),
                    objc2_core_graphics::kCGFloatingWindowLevel as isize
                );
                for (width, height) in [
                    (350.0, 230.0),
                    (1.0, 1.0),
                    (12.0, 120.0),
                    (26.0, 56.0),
                    (56.0, 26.0),
                ] {
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
                    assert!(native.contentMinSize().width <= 1.0);
                    assert!(native.contentMinSize().height <= 1.0);
                    assert_eq!(
                        native.contentRectForFrameRect(native.frame()),
                        native.frame()
                    );
                    assert!(!native.isVisible());
                    assert!(!native.isKeyWindow());
                    assert!(!app.isActive());
                }
            }
            drop(windows);
            let menu = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Snip2Pin · 操作")
                        .with_decorations(false)
                        .with_resizable(false)
                        .with_transparent(false)
                        .with_inner_size(LogicalSize::new(
                            snip2pin::ui::pin::MENU_SIZE.x,
                            snip2pin::ui::pin::MENU_SIZE.y,
                        ))
                        .with_active(false)
                        .with_visible(false),
                )
                .expect("hidden temporary action menu");
            let native_menu = app
                .windows()
                .into_iter()
                .find(|window| window.title().to_string() == "Snip2Pin · 操作")
                .expect("native menu");
            let menu_class = native_menu.class();
            platform::prepare_floating_windows().expect("prepare menu on the pin's Space");
            assert!(native_menu.collectionBehavior().contains(
                objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
                    | objc2_app_kit::NSWindowCollectionBehavior::FullScreenAuxiliary
            ));
            assert_eq!(
                native_menu.level(),
                objc2_core_graphics::kCGFloatingWindowLevel as isize
            );
            assert!(native_menu.level() < objc2_core_graphics::kCGDockWindowLevel as isize);
            assert!(native_menu.level() < objc2_core_graphics::kCGMainMenuWindowLevel as isize);
            assert_eq!(native_menu.class(), menu_class);
            assert!(!native_menu.isVisible());
            assert!(!native_menu.isKeyWindow());
            assert!(!app.isActive());
            drop(native_menu);
            drop(menu);

            // Exercise the production selection/placement chain and the same
            // egui-winit builder application used by eframe's native backend.
            // Generated source pixels never enter clipboard or capture APIs.
            let snapshot = capture::Snapshot {
                display: capture::Display {
                    id: 42,
                    origin: [0.0, 0.0],
                    size: [1000.0, 800.0],
                    scale: 2.0,
                },
                pixels: Arc::new(egui::ColorImage::filled([2000, 1600], egui::Color32::BLACK)),
            };
            let selected = geometry::selection_with_step(
                [100.5, 150.5],
                [400.5, 350.5],
                [1000.0, 800.0],
                snapshot.pixel_size(),
                platform::selection_pixel_step(&snapshot),
            )
            .expect("canonical Retina selection");
            assert_eq!(
                selected,
                geometry::PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402
                }
            );
            let placement = geometry::pin_placement(
                selected,
                snapshot.pixel_size(),
                snapshot.display.position(),
                snapshot.display.logical_size(),
            );
            let screens = objc2_app_kit::NSScreen::screens(thread);
            let primary = screens.iter().next().expect("native display").frame();
            let desktop_top = primary.origin.y + primary.size.height;
            let mut aligned_windows = Vec::new();
            for zoom in [1.0, 1.25, 2.0] {
                let ctx = egui::Context::default();
                ctx.set_zoom_factor(zoom);
                let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
                warmup.textures_delta.clear();
                let area = egui::Rect::from_min_size(
                    snapshot.display.position().into(),
                    snapshot.display.logical_size().into(),
                );
                let initial = pin::frame_geometry(
                    placement,
                    area,
                    pin::coordinate_ratio(
                        snapshot.display.scale,
                        zoom,
                        snapshot.display.scale * zoom,
                    ),
                )
                .image;
                let title = format!("Snip2Pin · 贴图 aligned-{zoom}");
                let builder = egui::ViewportBuilder::default()
                    .with_title(&title)
                    .with_decorations(false)
                    .with_transparent(false)
                    .with_has_shadow(true)
                    .with_position(initial.min)
                    .with_inner_size(initial.size())
                    .with_resizable(true)
                    .with_always_on_top()
                    .with_active(false)
                    .with_taskbar(false)
                    .with_visible(false);
                let image = event_loop
                    .create_window(egui_winit::create_winit_window_attributes(
                        &ctx,
                        builder.clone(),
                    ))
                    .expect("hidden canonical image window");
                egui_winit::apply_viewport_builder_to_window(&ctx, &image, &builder);
                platform::prepare_floating_windows().expect("prepare canonical image window");
                let position = image
                    .inner_position()
                    .expect("native content position")
                    .to_logical::<f64>(image.scale_factor());
                let size = image.inner_size().to_logical::<f64>(image.scale_factor());
                assert_eq!([position.x, position.y], [100.0, 150.0], "zoom={zoom}");
                assert_eq!([size.width, size.height], [301.0, 201.0], "zoom={zoom}");

                // Inspect raw AppKit doubles too: winit's physical integer
                // getters must not conceal a fractional native frame.
                let native = app
                    .windows()
                    .into_iter()
                    .find(|window| window.title().to_string() == title)
                    .expect("canonical native image");
                let content = native.contentRectForFrameRect(native.frame());
                let view = native.contentView().expect("content view");
                let view_screen =
                    native.convertRectToScreen(view.convertRect_toView(view.bounds(), None));
                assert_eq!(content, native.frame());
                assert_eq!(content, view_screen);
                assert_eq!(
                    [
                        content.origin.x,
                        desktop_top - content.origin.y - content.size.height
                    ],
                    [100.0, 150.0],
                    "raw origin, zoom={zoom}"
                );
                assert_eq!(
                    [content.size.width, content.size.height],
                    [301.0, 201.0],
                    "raw size, zoom={zoom}"
                );
                assert!(!native.isVisible());
                assert!(!native.isKeyWindow());
                assert!(!app.isActive());
                aligned_windows.push(image);
            }
            drop(aligned_windows);
            // Hidden windows do not reproduce AppKit's titlebar adjustment.
            // Order only small, fully transparent, input-ignoring test windows.
            for (index, screen) in screens.iter().enumerate() {
                let size = [188.0, 148.0];
                let visible = screen.visibleFrame();
                let position = [
                    (visible.origin.x + (visible.size.width - f64::from(size[0])) / 2.0).floor()
                        as f32,
                    (desktop_top - visible.origin.y - visible.size.height) as f32,
                ];
                let ctx = egui::Context::default();
                let title = format!("Snip2Pin · 贴图 ordered-{index}");
                let builder = egui::ViewportBuilder::default()
                    .with_title(&title)
                    .with_decorations(false)
                    .with_transparent(false)
                    .with_has_shadow(true)
                    .with_position(position)
                    .with_inner_size(size)
                    .with_resizable(true)
                    .with_always_on_top()
                    .with_active(false)
                    .with_taskbar(false)
                    .with_visible(false);
                let image = event_loop
                    .create_window(egui_winit::create_winit_window_attributes(
                        &ctx,
                        builder.clone(),
                    ))
                    .expect("initially hidden ordered test pin");
                egui_winit::apply_viewport_builder_to_window(&ctx, &image, &builder);
                let native = app
                    .windows()
                    .into_iter()
                    .find(|window| window.title().to_string() == title)
                    .expect("native ordered test pin");
                native.setAlphaValue(0.0);
                native.setIgnoresMouseEvents(true);
                let guard = TransparentOrder {
                    frame: native.frame(),
                    style: native.styleMask(),
                    window: native,
                };
                guard.assert_unchanged(&app);
                assert!(!guard.window.isVisible());
                assert_eq!(
                    guard.frame.origin.y + guard.frame.size.height,
                    visible.origin.y + visible.size.height,
                    "test pin starts directly below the menu bar"
                );
                assert_eq!(
                    [guard.frame.size.width, guard.frame.size.height],
                    size.map(f64::from)
                );
                guard.window.orderFront(None);
                guard.assert_unchanged(&app);
                let mut viewport_info = egui::ViewportInfo::default();
                egui_winit::update_viewport_info(&mut viewport_info, &ctx, &image, true);
                guard.assert_unchanged(&app);
                self.transparent_orders.push(guard);
                self.top_windows.push(image);
            }
            drop(images);
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            let thread = objc2::MainThreadMarker::new().expect("main thread");
            let app = objc2_app_kit::NSApplication::sharedApplication(thread);
            for guard in &self.transparent_orders {
                guard.assert_unchanged(&app);
            }
            self.transparent_orders.clear();
            self.top_windows.clear();
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
        .run_app(&mut Lifecycle::default())
        .expect("native lifecycle");
    println!(
        "macOS invisible native image lifecycle passed; visible live-resize remains unverified"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}
