//! Read only synthetic pixels from hidden windows' own GL backbuffers.
//! This cannot measure compositor flicker or a visible live drag.
#[cfg(target_os = "macos")]
#[path = "../src/platform/macos.rs"]
#[allow(dead_code)]
mod platform;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // GL creation/readback requires unsafe; no desktop capture is used.
mod regression {
    use std::{ffi::CString, num::NonZeroU32, sync::Arc};

    use eframe::{egui, egui_glow::Painter};
    use glow::HasContext;
    use glutin::{
        config::ConfigTemplateBuilder,
        context::ContextAttributesBuilder,
        display::{Display, DisplayApiPreference},
        prelude::*,
        surface::{SurfaceAttributesBuilder, WindowSurface},
    };
    use objc2_app_kit::{NSApplication, NSScreen};
    use snip2pin::{geometry::Placement, pin::Frame};
    use winit::{
        application::ApplicationHandler,
        dpi::LogicalSize,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS},
        raw_window_handle::{HasDisplayHandle, HasWindowHandle},
        window::{Window, WindowId},
    };

    const PIN_ID: u64 = 982_451_653;
    const COLORS: [[u8; 4]; 4] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
    ];

    fn nonzero(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).expect("nonempty hidden drawable")
    }

    /// Clear four image quadrants, then sample outer corners and the transparent
    /// host margin. Coordinates are converted from egui's top-left origin.
    unsafe fn draw_and_read(gl: &glow::Context, frame: Frame, size: [u32; 2], scale: f32) -> bool {
        let image = frame.image;
        let center = image.center();
        let quadrants = [
            egui::Rect::from_min_max(image.min, center),
            egui::Rect::from_min_max(
                egui::pos2(center.x, image.min.y),
                egui::pos2(image.max.x, center.y),
            ),
            egui::Rect::from_min_max(
                egui::pos2(image.min.x, center.y),
                egui::pos2(center.x, image.max.y),
            ),
            egui::Rect::from_min_max(center, image.max),
        ];
        // SAFETY: the calling handler owns the current context on the main thread.
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.draw_buffer(glow::BACK);
            gl.read_buffer(glow::BACK);
            gl.disable(glow::FRAMEBUFFER_SRGB);
            gl.enable(glow::SCISSOR_TEST);
            for (rect, color) in quadrants.iter().zip(COLORS) {
                let x0 = (rect.min.x * scale).round() as i32;
                let x1 = (rect.max.x * scale).round() as i32;
                let y0 = (rect.min.y * scale).round() as i32;
                let y1 = (rect.max.y * scale).round() as i32;
                gl.scissor(x0, size[1] as i32 - y1, x1 - x0, y1 - y0);
                gl.clear_color(
                    color[0] as f32 / 255.0,
                    color[1] as f32 / 255.0,
                    color[2] as f32 / 255.0,
                    1.0,
                );
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
            gl.disable(glow::SCISSOR_TEST);
            gl.finish();
            let mut valid = true;
            let inset = egui::vec2(2.0, 2.0);
            let corners = [
                image.min + inset,
                egui::pos2(image.max.x - 2.0, image.min.y + 2.0),
                egui::pos2(image.min.x + 2.0, image.max.y - 2.0),
                image.max - inset,
            ];
            for (point, expected) in corners
                .into_iter()
                .zip(COLORS)
                .chain(std::iter::once((egui::pos2(1.0, 1.0), [0; 4])))
            {
                let mut pixel = [0u8; 4];
                gl.read_pixels(
                    (point.x * scale) as i32,
                    size[1] as i32 - 1 - (point.y * scale) as i32,
                    1,
                    1,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut pixel)),
                );
                valid &= pixel == expected;
            }
            let error = gl.get_error();
            valid && error == glow::NO_ERROR
        }
    }

    fn refresh(painter: &mut Painter, frame: Frame, size: [u32; 2], scale: f32) {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, frame.window.size());
        painter.paint_primitives(
            size,
            scale,
            &[egui::ClippedPrimitive {
                clip_rect: bounds,
                primitive: egui::epaint::Primitive::Callback(super::platform::drawable_callback(
                    PIN_ID, bounds,
                )),
            }],
        );
    }

    struct Regression;
    impl ApplicationHandler for Regression {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let windows: Vec<_> = (0..2)
                .map(|index| {
                    event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title(format!("Snip2Pin · 贴图 {}", PIN_ID + index))
                                .with_decorations(false)
                                .with_transparent(true)
                                .with_visible(false)
                                .with_inner_size(LogicalSize::new(260.0, 200.0)),
                        )
                        .expect("create hidden synthetic window")
                })
                .collect();
            let thread = objc2::MainThreadMarker::new().expect("main thread");
            let app = NSApplication::sharedApplication(thread);
            let native = app
                .windows()
                .into_iter()
                .find(|window| window.title().to_string() == format!("Snip2Pin · 贴图 {PIN_ID}"))
                .expect("own hidden pin window");
            let screen = NSScreen::screens(thread)
                .firstObject()
                .expect("primary screen");
            let full = screen.frame();
            let visible = screen.visibleFrame();
            let area = egui::Rect::from_min_size(
                egui::pos2(
                    visible.origin.x as f32,
                    (full.origin.y + full.size.height - visible.origin.y - visible.size.height)
                        as f32,
                ),
                egui::vec2(visible.size.width as f32, visible.size.height as f32),
            );
            let original = snip2pin::pin::frame_geometry(
                Placement {
                    position: [area.min.x + 180.0, area.min.y + 180.0],
                    size: [240.0, 160.0],
                },
                area,
                1.0,
                1.0,
            );
            super::platform::layout_pin(&native, original).expect("initial native geometry");

            // SAFETY: both handles remain alive until the surfaces and context drop.
            let display = unsafe {
                Display::new(
                    windows[0].display_handle().unwrap().as_raw(),
                    DisplayApiPreference::Cgl,
                )
            }
            .expect("CGL display");
            let template = ConfigTemplateBuilder::new()
                .with_depth_size(0)
                .with_stencil_size(0)
                .with_transparency(true)
                .build();
            let config = unsafe { display.find_configs(template) }
                .expect("CGL configs")
                .next()
                .expect("CGL config");
            let attrs = ContextAttributesBuilder::new()
                .build(Some(windows[0].window_handle().unwrap().as_raw()));
            let context = unsafe { display.create_context(&config, &attrs) }.expect("CGL context");
            let surfaces: Vec<_> = windows
                .iter()
                .map(|window| {
                    let size = window.inner_size();
                    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
                        window.window_handle().unwrap().as_raw(),
                        nonzero(size.width),
                        nonzero(size.height),
                    );
                    unsafe { display.create_window_surface(&config, &attrs) }
                        .expect("hidden CGL surface")
                })
                .collect();
            let context = context
                .make_current(&surfaces[0])
                .expect("bind first surface");
            let gl = Arc::new(unsafe {
                glow::Context::from_loader_function(|name| {
                    display.get_proc_address(&CString::new(name).unwrap())
                })
            });
            let mut painter = Painter::new(gl.clone(), "", None, false).expect("Glow painter");
            let scale = windows[0].scale_factor() as f32;
            let size = windows[0].inner_size();
            surfaces[0].resize(&context, nonzero(size.width), nonzero(size.height));
            painter.clear([size.width, size.height], [0.0; 4]);
            refresh(&mut painter, original, [size.width, size.height], scale);
            let available =
                unsafe { draw_and_read(&gl, original, [size.width, size.height], scale) };
            if available {
                let mut a_failures = [0usize; 2];
                let mut cases = 0;
                for switch_surface in [false, true] {
                    for (dx, dy) in [
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ] {
                        for fixed in [false, true] {
                            // Each A/B attempt starts from the same known drawable.
                            context.make_current(&surfaces[0]).expect("reset context");
                            super::platform::layout_pin(&native, original).unwrap();
                            let size = windows[0].inner_size();
                            surfaces[0].resize(&context, nonzero(size.width), nonzero(size.height));
                            painter.clear([size.width, size.height], [0.0; 4]);
                            assert!(
                                unsafe {
                                    draw_and_read(&gl, original, [size.width, size.height], scale)
                                },
                                "reset drawable must remain usable"
                            );
                            if switch_surface {
                                context
                                    .make_current(&surfaces[1])
                                    .expect("previous viewport");
                            }
                            let mut image = original.image.translate(original.window.min.to_vec2());
                            if dx < 0 {
                                image.min.x -= 80.0;
                            } else if dx > 0 {
                                image.max.x += 80.0;
                            }
                            if dy < 0 {
                                image.min.y -= 60.0;
                            } else if dy > 0 {
                                image.max.y += 60.0;
                            }
                            let frame = original.resized_to(image, &[area], 1.0);
                            assert_eq!(
                                frame.image.translate(frame.window.min.to_vec2()),
                                image,
                                "resizing preserves the opposite image anchor"
                            );
                            // Still inside this handler: queued winit Resized cannot run.
                            let (actual, changed) =
                                super::platform::layout_pin(&native, frame).unwrap();
                            assert!(changed);
                            assert_eq!(actual, frame.window);
                            if switch_surface {
                                context.make_current(&surfaces[0]).expect("target viewport");
                            }
                            let size = windows[0].inner_size();
                            let size = [size.width, size.height];
                            painter.clear(size, [0.0; 4]);
                            if fixed {
                                refresh(&mut painter, frame, size, scale);
                            }
                            let valid = unsafe { draw_and_read(&gl, frame, size, scale) };
                            if fixed {
                                assert!(
                                    valid,
                                    "B: update+clear failed, switch={switch_surface}, direction=({dx},{dy})"
                                );
                            } else {
                                a_failures[usize::from(switch_surface)] += usize::from(!valid);
                                cases += 1;
                            }
                        }
                    }
                }
                println!(
                    "macOS hidden drawable A/B: B passed {cases} cases; A mismatches: same surface={}, switched surface={} (0 means stale drawable not reproduced). This does not verify compositor flicker.",
                    a_failures[0], a_failures[1]
                );
            } else {
                println!(
                    "UNAVAILABLE: macOS did not provide a readable synthetic backbuffer for fully hidden windows. A/B not executed; visible resize flicker remains unverified."
                );
            }
            assert!(!native.isVisible());
            assert!(!app.isActive());
            for window in &windows {
                assert_eq!(window.is_visible(), Some(false));
            }
            painter.destroy();
            drop(painter);
            drop(gl);
            drop(context);
            drop(surfaces);
            drop(windows);
            event_loop.exit();
        }

        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }

    pub fn run() {
        let event_loop = EventLoop::builder()
            .with_activation_policy(ActivationPolicy::Prohibited)
            .with_activate_ignoring_other_apps(false)
            .build()
            .expect("hidden main-thread event loop");
        event_loop
            .run_app(&mut Regression)
            .expect("hidden drawable regression");
    }
}

#[cfg(target_os = "macos")]
fn main() {
    regression::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
