use super::*;

fn app() -> Snip2Pin {
    let (sender, events) = mpsc::channel();
    Snip2Pin {
        sender,
        events,
        phase: Phase::Idle,
        gate: CaptureGate::default(),
        layers: BTreeMap::new(),
        active_display: None,
        pins: BTreeMap::new(),
        next_pin: 0,
        message: None,
        shortcut: None,
        preferences: Preferences::default(),
        applied_icon: None,
        icon_data: None,
        icon_texture: None,
        config_path: None,
        recording: false,
        suppress_shortcut_until: Instant::now(),
        demo: true,
        tray: None,
        startup: None,
        startup_enabled: false,
        exiting: false,
        home_visible: true,
        booting: false,
        permission: ScreenPermission::default(),
        relaunch: Arc::new(Mutex::new(None)),
    }
}

#[test]
fn missing_permission_keeps_capture_idle_and_opens_an_actionable_home() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.demo = false;
    app.home_visible = false;
    let mut prompts = 0;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        assert!(!app.check_capture_permission(
            ctx,
            || false,
            || {
                prompts += 1;
                false
            }
        ));
        assert!(!app.check_capture_permission(ctx, || false, || panic!("no repeated prompt")));
    });
    assert_eq!(prompts, 1);
    assert!(matches!(app.phase, Phase::Idle));
    assert_eq!(app.permission, ScreenPermission::Required);
    assert!(app.home_visible);
    assert!(app.layers.is_empty());
    assert!(app.pins.is_empty());
    assert!(
        app.gate.begin().is_some(),
        "permission failure cannot occupy the capture gate"
    );
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Visible(true)));
    assert!(app.check_capture_permission(&ctx, || true, || panic!("already granted")));
    assert_eq!(app.permission, ScreenPermission::Granted);
}

#[test]
fn demo_never_requests_permission_or_relaunches_the_real_application() {
    let ctx = egui::Context::default();
    let mut app = app();
    assert!(app.check_capture_permission(
        &ctx,
        || panic!("demo must not preflight"),
        || panic!("demo must not request")
    ));
    app.restart(&ctx);
    assert!(!app.exiting);
    assert!(app.relaunch.lock().expect("relaunch handle").is_none());
}

#[test]
fn background_bootstrap_keeps_capture_layers_available_without_painting_home() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    app.home_visible = false;
    app.booting = true;
    let display = capture::demo().display;
    let display_id = display.id;
    app.layers.insert(
        display_id,
        Layer {
            display,
            selection: None,
            positioned: Arc::new(AtomicBool::new(false)),
        },
    );
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.show_layers(ui.ctx());
        assert!(!app.prepare_home(ui.ctx()));
    });
    output.textures_delta.clear();
    assert!(
        output.shapes.is_empty(),
        "background startup has no visible home contents"
    );
    assert!(!app.booting);
    assert!(
        matches!(app.phase, Phase::Idle),
        "startup never captures the desktop"
    );
    assert!(
        output.viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::Visible(false))
    );
    let layer = &output.viewport_output[&layer_id(display_id)];
    assert_eq!(layer.builder.mouse_passthrough, Some(true));
    assert_eq!(layer.builder.active, Some(false));
    assert_ne!(layer.builder.fullscreen, Some(true));
}

#[test]
fn tray_home_and_quit_dispatch_root_commands_and_demo_cannot_enable_startup() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.home_visible = false;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Home, ctx)
    });
    assert!(app.home_visible);
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Visible(true)));
    app.set_startup(true);
    assert!(!app.startup_enabled, "demo never registers startup");
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx)
    });
    assert!(app.exiting);
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Close));
}

#[test]
fn capture_never_switches_the_desktop_into_native_fullscreen() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_capture(id, Ok(vec![capture::demo()]), None, ui.ctx());
        app.show_layers(ui.ctx());
        app.restore(ui.ctx());
    });
    output.textures_delta.clear();
    assert!(
        output.viewport_output.values().all(|viewport| {
            viewport.builder.fullscreen != Some(true)
                && viewport.builder.monitor.is_none()
                && !viewport.commands.iter().any(|command| {
                    matches!(
                        command,
                        ViewportCommand::Fullscreen(_) | ViewportCommand::SetMonitor(_)
                    )
                })
        }),
        "capture must never enter/leave native fullscreen or switch monitors"
    );
}

#[test]
fn selection_layer_does_not_retry_native_position_changes() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    let viewport = layer_id(0);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("selection callback");
    for index in 0..3 {
        let mut input = egui::RawInput {
            viewport_id: viewport,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(680.0, 420.0),
            )),
            ..Default::default()
        };
        input.viewports.insert(
            viewport,
            egui::ViewportInfo {
                outer_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(84.0, 104.0),
                    egui::vec2(680.0, 420.0),
                )),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| callback(ui));
        output.textures_delta.clear();
        let count = output.viewport_output[&viewport]
            .commands
            .iter()
            .filter(|command| matches!(command, ViewportCommand::OuterPosition(_)))
            .count();
        assert_eq!(count, usize::from(!cfg!(target_os = "macos") && index == 0));
    }
}

#[test]
fn capture_initially_highlights_the_display_under_the_cursor() {
    let ctx = egui::Context::default();
    let mut app = app();
    let mut left = capture::demo();
    left.display.id = 9;
    left.display.origin = [-680.0, -100.0];
    left.display.scale = 2.0;
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(
        id,
        Ok(vec![capture::demo(), left]),
        Some([-300.0, -50.0]),
        &ctx,
    );
    assert_eq!(
        app.active_display,
        Some(9),
        "the pointer chooses the screen, not display ordering or the launcher"
    );
}

#[test]
fn every_monitor_gets_a_layer_and_pin_uses_that_monitors_origin() {
    let ctx = egui::Context::default();
    let mut app = app();
    let mut left = capture::demo();
    left.display.id = 9;
    left.display.origin = [-680.0, -100.0];
    let origin = left.display.position();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo(), left]), None, &ctx);
    assert_eq!(app.layers.len(), 2);
    assert_eq!(app.active_display, Some(0));
    app.selection_action(id, 9, overlay::Action::Activate, &ctx);
    assert_eq!(app.active_display, Some(9));
    app.selection_action(
        id,
        9,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let pin = app.pins.values().next().expect("pin created");
    assert_eq!(pin.placement.position, [origin[0] + 68.0, origin[1] + 42.0]);
    assert_eq!(pin.placement.size, [136.0, 84.0]);
    assert!(matches!(
        app.phase,
        Phase::Selecting {
            pending_pin: Some(_),
            ..
        }
    ));
    app.restore(&ctx);
    assert_eq!(app.active_display, None);
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
    assert_eq!(app.pins.len(), 1);
}

#[test]
fn canceled_and_stale_selections_release_buffers_without_creating_pins() {
    let ctx = egui::Context::default();
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(id + 1, 0, overlay::Action::Cancel, &ctx);
    assert!(matches!(app.phase, Phase::Selecting { .. }));
    app.selection_action(id, 0, overlay::Action::Cancel, &ctx);
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
    assert!(app.pins.is_empty());
}

#[test]
fn successful_selection_copies_only_cropped_pixels_once_and_can_copy_again() {
    let ctx = egui::Context::default();
    let mut app = app();
    let snapshot = capture::demo();
    let rect = geometry::PixelRect {
        x: 3,
        y: 5,
        width: 7,
        height: 9,
    };
    let expected = color_image(&capture::crop(&snapshot.pixels, rect).expect("valid crop"));
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.selection_action(id + 1, 0, overlay::Action::Select(rect), ui.ctx());
        assert!(
            app.pins.is_empty(),
            "stale selections cannot copy or create pins"
        );
        app.selection_action(id, 0, overlay::Action::Select(rect), ui.ctx());
        app.selection_action(id, 0, overlay::Action::Select(rect), ui.ctx());
    });
    output.textures_delta.clear();
    assert_eq!(output.platform_output.commands.len(), 1);
    assert!(
        matches!(&output.platform_output.commands[0], egui::OutputCommand::CopyImage(image) if image == &expected)
    );
    assert_eq!(app.pins[&1].pixels.as_ref(), &expected);

    app.pins[&1]
        .prepared
        .lock()
        .expect("pin geometry")
        .set_size(egui::vec2(70.0, 45.0), 2.0);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    output.textures_delta.clear();
    assert_eq!(output.platform_output.commands.len(), 1);
    assert!(
        matches!(&output.platform_output.commands[0], egui::OutputCommand::CopyImage(image) if image == &expected)
    );
    app.pins.remove(&1);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    output.textures_delta.clear();
    assert!(
        output.platform_output.commands.is_empty(),
        "closed pins cannot overwrite the clipboard"
    );
}

#[test]
fn canceled_and_invalid_selections_leave_the_clipboard_untouched() {
    for rect in [
        None,
        Some(geometry::PixelRect {
            x: 0,
            y: 0,
            width: 0,
            height: 1,
        }),
        Some(geometry::PixelRect {
            x: u32::MAX,
            y: 1,
            width: 1,
            height: 1,
        }),
    ] {
        let ctx = egui::Context::default();
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.accept_capture(id, Ok(vec![capture::demo()]), None, ui.ctx());
            app.selection_action(
                id,
                0,
                rect.map_or(overlay::Action::Cancel, overlay::Action::Select),
                ui.ctx(),
            );
        });
        output.textures_delta.clear();
        assert!(output.platform_output.commands.is_empty());
        assert!(app.pins.is_empty());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn menu_bar_constrained_layer_waits_before_painting_the_full_desktop() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut setup = ctx.run_ui(
        egui::RawInput {
            max_texture_side: Some(4096),
            ..Default::default()
        },
        |_| {},
    );
    setup.textures_delta.clear();
    let mut app = app();
    app.demo = false;
    let mut snapshot = capture::demo();
    snapshot.display.size = [1440.0, 900.0];
    snapshot.pixels = image::RgbaImage::new(2880, 1800);
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
    let layer = layer_id(0);
    let texture = app.layers[&0]
        .selection
        .as_ref()
        .expect("selection")
        .lock()
        .expect("selection lock")
        .texture
        .id();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
    output.textures_delta.clear();
    let paint = output.viewport_output[&layer]
        .viewport_ui_cb
        .as_ref()
        .expect("native layer callback");
    for (size, should_paint) in [([1440.0, 870.0], false), ([1440.0, 900.0], true)] {
        let mut input = egui::RawInput {
            viewport_id: layer,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size.into())),
            ..Default::default()
        };
        input.viewports.insert(layer, egui::ViewportInfo::default());
        let mut frame = ctx.run_ui(input, |ui| paint(ui));
        frame.textures_delta.clear();
        let images: Vec<_> = frame
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
                _ => None,
            })
            .collect();
        if should_paint {
            assert_eq!(
                images,
                vec![egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 900.0)
                )],
                "Retina desktop must retain its full dimensions and aspect ratio"
            );
        } else {
            assert!(
                images.is_empty(),
                "never paint a vertically compressed desktop"
            );
        }
    }
}

#[test]
fn mismatched_native_position_is_not_retried_and_keeps_image_pixels_in_place() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        id,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let viewport = ViewportId::from_hash_of(("pin", 1_u64));
    let texture = app.pins[&1].texture.id();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    output.textures_delta.clear();
    let paint = output.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    for index in 0..3 {
        let pin = &app.pins[&1];
        let window = ui::pin::frame_geometry(pin.placement, pin.screen, 1.0, 1.0).window;
        let rect = window.translate(egui::vec2(4.0, 3.0));
        let mut input = egui::RawInput {
            viewport_id: viewport,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size())),
            ..Default::default()
        };
        input.viewports.insert(
            viewport,
            egui::ViewportInfo {
                inner_rect: Some(rect),
                outer_rect: Some(rect),
                ..Default::default()
            },
        );
        let mut frame = ctx.run_ui(input, |ui| paint(ui));
        frame.textures_delta.clear();
        let commands = &frame.viewport_output[&viewport].commands;
        assert_eq!(
            commands
                .iter()
                .filter(|command| matches!(
                    command,
                    ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
                ))
                .count(),
            if index == 0 { 2 } else { 0 },
            "geometry is configured once, never retried"
        );
        let image = frame.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
            _ => None,
        });
        if index == 0 {
            assert!(image.is_none());
        } else {
            let image = image.expect("native position cannot block the captured image");
            let expected = egui::pos2(pin.placement.position[0], pin.placement.position[1]);
            assert!((rect.min + image.min.to_vec2()).distance(expected) < 0.001);
            assert_eq!(
                image.size(),
                egui::vec2(pin.placement.size[0], pin.placement.size[1])
            );
        }
        assert_eq!(
            app.events
                .try_iter()
                .any(|event| matches!(event, Event::PinReady(_, _))),
            index == 1
        );
    }
}

#[test]
fn top_edge_pin_finishes_capture_when_the_system_moves_it_below_the_menu_bar() {
    for height in [120, 600] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        let mut snapshot = capture::demo();
        snapshot.display.origin = [0.0, 0.0];
        app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
        app.selection_action(
            id,
            0,
            overlay::Action::Select(geometry::PixelRect {
                x: 96,
                y: 0,
                width: 192,
                height,
            }),
            &ctx,
        );
        let viewport = ViewportId::from_hash_of(("pin", 1_u64));
        let texture = app.pins[&1].texture.id();
        let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
        setup.textures_delta.clear();
        let output = &setup.viewport_output[&viewport];
        let paint = output.viewport_ui_cb.as_ref().expect("pin callback");
        let mut actual = egui::Rect::from_min_size(
            output.builder.position.expect("position"),
            output.builder.inner_size.expect("size"),
        );
        let mut painted = false;
        let mut ready = false;
        for _ in 0..4 {
            actual = actual.translate(egui::vec2(0.0, (28.0 - actual.min.y).max(0.0)));
            let mut input = egui::RawInput {
                viewport_id: viewport,
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
                ..Default::default()
            };
            input.viewports.insert(
                viewport,
                egui::ViewportInfo {
                    inner_rect: Some(actual),
                    outer_rect: Some(actual),
                    ..Default::default()
                },
            );
            let mut frame = ctx.run_ui(input, |ui| paint(ui));
            frame.textures_delta.clear();
            painted |= frame.shapes.iter().any(|shape| {
                matches!(&shape.shape,
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture)
            });
            ready |= app
                .events
                .try_iter()
                .any(|event| matches!(event, Event::PinReady(session, 1) if session == id));
            for command in &frame.viewport_output[&viewport].commands {
                match command {
                    ViewportCommand::InnerSize(size) => {
                        actual = egui::Rect::from_min_size(actual.min, *size)
                    }
                    ViewportCommand::OuterPosition(position) => {
                        actual = egui::Rect::from_min_size(*position, actual.size())
                    }
                    _ => {}
                }
            }
        }
        assert!(
            ready && painted,
            "top-edge capture must reveal the moved pin and finish, instead of endlessly retrying an impossible desktop position"
        );
        app.restore(&ctx);
        assert!(matches!(app.phase, Phase::Idle));
        assert!(app.layers.values().all(|layer| layer.selection.is_none()));
    }
}

#[test]
fn image_bounds_switch_toolbar_for_initial_capture_and_dragging() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    let mut left = capture::demo();
    left.display.id = 9;
    left.display.scale = 2.0;
    left.display.origin = [-680.0, -100.0];
    app.accept_capture(id, Ok(vec![capture::demo(), left]), None, &ctx);
    app.selection_action(
        id,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let pin = &app.pins[&1];
    let initial = platform::pin_host(ui::pin::frame_geometry(pin.placement, pin.screen, 1.0, 1.0));
    let texture = pin.texture.id();
    let viewport = ViewportId::from_hash_of(("pin", 1_u64));
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    let top = egui::pos2(initial.window.min.x, pin.screen.min.y - initial.image.min.y);
    let left_screen = pin
        .screen_areas
        .iter()
        .find(|screen| screen.min.x < pin.screen.min.x)
        .expect("left screen");
    let left_top = left_screen.min + initial.image.size() / 2.0;
    let left_top = egui::pos2(left_top.x, left_screen.min.y - initial.image.min.y);
    let enough_space = egui::vec2(0.0, snip2pin::pin::METRICS.toolbar_height() * 3.0);
    for (index, (position, below)) in [
        (top, true),
        (top, true),
        (top + enough_space, false),
        (top, true),
        (left_top + enough_space, false),
        (left_top, true),
    ]
    .into_iter()
    .enumerate()
    {
        let expected = platform::pin_host(initial.moved_to(
            position + initial.image.min.to_vec2(),
            &pin.screen_areas,
            1.0,
        ));
        let actual = expected.window;
        // This callback paints after the drag's commands have been applied by
        // the native backend. Pointer command generation is covered in ui/pin.
        pin.prepared.lock().expect("pin state").drag.set_position(
            expected.image.min + actual.min.to_vec2(),
            &Coordinates::new(1.0, 1.0, 1.0),
        );
        let mut input = egui::RawInput {
            viewport_id: viewport,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
            events: vec![egui::Event::PointerMoved(initial.image.center())],
            ..Default::default()
        };
        input.viewports.insert(
            viewport,
            egui::ViewportInfo {
                inner_rect: Some(actual),
                outer_rect: Some(actual),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| paint(ui));
        output.textures_delta.clear();
        if index == 0 {
            continue;
        }
        assert!(
            output.viewport_output[&viewport]
                .commands
                .iter()
                .all(|command| !matches!(
                    command,
                    ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
                ))
        );
        let image = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
                _ => None,
            })
            .expect("image stays visible");
        assert_eq!(
            image, expected.image,
            "painted pixels follow the image position after native margins change"
        );
        let close = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "×" => Some((
                    shape.clip_rect,
                    egui::Rect::from_min_size(text.pos, text.galley.size()),
                )),
                _ => None,
            })
            .expect("hover reveals close button");
        assert!(
            close.0.contains_rect(close.1),
            "both toolbar rows must fit the native surface"
        );
        assert_eq!(
            close.1.min.y > image.max.y,
            below,
            "step {index}, native={actual:?}, image={image:?}"
        );
        assert!(!close.1.intersects(image));
    }
}

#[test]
fn pending_pin_can_be_closed_before_native_geometry_is_ready() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        id,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let viewport = ViewportId::from_hash_of(("pin", 1_u64));
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    let mut input = egui::RawInput {
        viewport_id: viewport,
        ..Default::default()
    };
    input.viewports.insert(
        viewport,
        egui::ViewportInfo {
            events: vec![egui::ViewportEvent::Close],
            ..Default::default()
        },
    );
    let mut frame = ctx.run_ui(input, |ui| paint(ui));
    frame.textures_delta.clear();
    assert!(matches!(app.events.try_recv(), Ok(Event::ClosePin(1))));
    app.close_pin(1, &ctx);
    assert!(app.pins.is_empty());
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
}

#[test]
fn selection_escape_cancels_a_pending_pin_and_releases_capture_layers() {
    let ctx = egui::Context::default();
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        id,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    assert_eq!(app.pins.len(), 1);
    app.selection_action(id, 0, overlay::Action::Cancel, &ctx);
    assert!(app.pins.is_empty());
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
}

#[test]
fn aligned_pin_keeps_image_origin_across_dpi_zoom_and_native_frame_insets() {
    for scale in [1.0, 1.25, 2.0] {
        for zoom in [1.0, 1.25] {
            for origin in [[80.0, 100.0], [-680.0, -100.0]] {
                let ctx = egui::Context::default();
                ctx.set_embed_viewports(false);
                ctx.set_zoom_factor(zoom);
                let mut app = app();
                let id = app.gate.begin().expect("capture starts");
                let mut snapshot = capture::demo();
                snapshot.display.origin = origin;
                snapshot.display.scale = scale;
                if cfg!(target_os = "macos") {
                    snapshot.display.size = [960.0 / scale, 600.0 / scale];
                } else {
                    snapshot.display.origin = origin.map(|value| value * scale);
                    snapshot.display.size = [960.0, 600.0];
                }
                app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
                app.selection_action(
                    id,
                    0,
                    overlay::Action::Select(geometry::PixelRect {
                        x: 97,
                        y: 61,
                        width: 193,
                        height: 121,
                    }),
                    &ctx,
                );
                let viewport = ViewportId::from_hash_of(("pin", 1_u64));
                let pin = &app.pins[&1];
                let texture = pin.texture.id();
                let position =
                    egui::pos2(pin.placement.position[0], pin.placement.position[1]) / zoom;
                let size = egui::vec2(pin.placement.size[0], pin.placement.size[1]) / zoom;
                let grid = if cfg!(target_os = "macos") {
                    zoom
                } else {
                    scale * zoom
                };
                let window = platform::pin_host(ui::pin::frame_geometry(
                    pin.placement,
                    pin.screen,
                    zoom.recip(),
                    grid,
                ))
                .window;
                let inset = egui::vec2(4.0, 7.0) / (scale * zoom);
                let mut output =
                    ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
                output.textures_delta.clear();
                let paint = output.viewport_output[&viewport]
                    .viewport_ui_cb
                    .as_ref()
                    .expect("pin callback");
                let input = |rect: egui::Rect| {
                    let mut input = egui::RawInput {
                        viewport_id: viewport,
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size())),
                        ..Default::default()
                    };
                    input.viewports.insert(
                        viewport,
                        egui::ViewportInfo {
                            native_pixels_per_point: Some(scale),
                            inner_rect: Some(rect),
                            outer_rect: Some(rect.translate(-inset)),
                            ..Default::default()
                        },
                    );
                    input
                };
                let mut first = ctx.run_ui(input(window), |ui| paint(ui));
                first.textures_delta.clear();
                let commands = &first.viewport_output[&viewport].commands;
                let resize = commands
                    .iter()
                    .position(|command| matches!(command, ViewportCommand::InnerSize(_)))
                    .expect("resize");
                let move_window = commands
                    .iter()
                    .position(|command| matches!(command, ViewportCommand::OuterPosition(_)))
                    .expect("position");
                assert!(
                    resize < move_window,
                    "resizing must not undo the final top-left position"
                );
                assert!(commands.iter().any(|command| matches!(command,
                    ViewportCommand::OuterPosition(actual) if actual.distance(window.min - inset) < 0.001
                )), "position targets the client area, compensating native insets");
                let mut frame = ctx.run_ui(input(window), |ui| paint(ui));
                frame.textures_delta.clear();
                let image = frame
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                            Some(mesh.calc_bounds())
                        }
                        _ => None,
                    })
                    .expect("aligned image is painted");
                let tolerance = 0.001;
                assert!((window.min + image.min.to_vec2()).distance(position) < tolerance);
                assert!(
                    (image.width() - size.x).abs() < tolerance
                        && (image.height() - size.y).abs() < tolerance,
                    "native window rounding must not stretch the image"
                );
                assert!(
                    app.events
                        .try_iter()
                        .any(|event| matches!(event, Event::PinReady(_, 1)))
                );
                let mut dragged = ctx
                    .run_ui(input(window.translate(egui::vec2(20.0, 30.0))), |ui| {
                        paint(ui)
                    });
                dragged.textures_delta.clear();
                assert!(
                    dragged.viewport_output[&viewport]
                        .commands
                        .iter()
                        .all(|command| !matches!(command, ViewportCommand::OuterPosition(_))),
                    "once initialized, user dragging must not snap the pin back"
                );
            }
        }
    }
}

#[test]
fn pin_window_is_created_at_the_selection_origin_without_changing_root() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        id,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.show_pins(ui.ctx());
    });
    output.textures_delta.clear();
    let pin = output
        .viewport_output
        .get(&ViewportId::from_hash_of(("pin", 1_u64)))
        .expect("native pin");
    let model = &app.pins[&1];
    let layout = ui::pin::frame_geometry(model.placement, model.screen, 1.0, 1.0);
    assert_eq!(pin.builder.position, Some(layout.window.min));
    assert_eq!(pin.builder.inner_size, Some(layout.window.size()));
    assert!(
        output.viewport_output[&ViewportId::ROOT]
            .commands
            .iter()
            .all(|command| matches!(command, ViewportCommand::SetTheme(_))),
        "unexpected root commands: {:?}",
        output.viewport_output[&ViewportId::ROOT].commands
    );
}

#[test]
fn ready_and_cancel_repaint_warmed_selection_layers_without_pointer_events() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    app.layers.insert(
        0,
        Layer {
            display: capture::demo().display,
            selection: None,
            positioned: Arc::new(AtomicBool::new(false)),
        },
    );
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
    output.textures_delta.clear();
    let drain = || {
        for _ in 0..4 {
            let mut input = egui::RawInput {
                viewport_id: layer_id(0),
                ..Default::default()
            };
            input.viewports.insert(
                layer_id(0),
                egui::ViewportInfo {
                    parent: Some(ViewportId::ROOT),
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |_| {});
            output.textures_delta.clear();
        }
    };
    drain();
    let repaints = Arc::new(Mutex::new(Vec::new()));
    let observed = repaints.clone();
    ctx.set_request_repaint_callback(move |info| {
        observed.lock().expect("repaint log").push(info.viewport_id);
    });
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    assert!(
        repaints.lock().expect("repaint log").contains(&layer_id(0)),
        "ready must paint the selection without waiting for pointer input"
    );
    drain();
    repaints.lock().expect("repaint log").clear();
    app.restore(&ctx);
    assert!(
        repaints.lock().expect("repaint log").contains(&layer_id(0)),
        "cancel must clear the selection without waiting for pointer input"
    );
}

#[test]
fn appearance_changes_preserve_shortcut_and_survive_restart() {
    let directory = tempfile::tempdir().expect("temporary preferences");
    let path = directory.path().join("preferences.toml");
    let ctx = egui::Context::default();
    let mut app = app();
    app.config_path = Some(path.clone());
    app.preferences.binding = Binding::parse("Ctrl+Alt+K").expect("custom binding");
    app.change_appearance(settings::Theme::Dark, settings::Icon::Line, &ctx);
    assert_eq!(
        app.preferences.binding,
        Binding::parse("Ctrl+Alt+K").expect("preserved binding")
    );
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(Preferences::load(&path).expect("restart"), app.preferences);
    app.change_binding(Binding::parse("Ctrl+Shift+4").expect("new binding"), &ctx);
    let reloaded = Preferences::load(&path).expect("restart after shortcut change");
    assert_eq!(reloaded.theme, settings::Theme::Dark);
    assert_eq!(reloaded.icon, settings::Icon::Line);
    assert_eq!(
        reloaded.binding,
        Binding::parse("Ctrl+Shift+4").expect("updated binding")
    );
}

#[test]
fn failed_preference_save_leaves_theme_and_icon_unchanged() {
    let directory = tempfile::tempdir().expect("temporary preferences");
    let ctx = egui::Context::default();
    let mut app = app();
    app.config_path = Some(directory.path().to_path_buf());
    let previous = app.preferences;
    let previous_theme = ctx.theme();
    app.change_appearance(settings::Theme::Light, settings::Icon::Metal, &ctx);
    assert_eq!(app.preferences, previous);
    assert_eq!(ctx.theme(), previous_theme);
    assert!(app.message.is_some());
    assert_eq!(
        std::fs::read_dir(directory.path())
            .expect("directory preserved")
            .count(),
        0
    );
}
