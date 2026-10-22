use super::*;

fn key_tap(key: egui::Key) -> [egui::Event; 2] {
    [true, false].map(|pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    })
}

fn input(id: u64, rect: egui::Rect, scale: f32, events: Vec<egui::Event>) -> egui::RawInput {
    let mut input = egui::RawInput {
        viewport_id: pin_id(id),
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size())),
        events,
        ..Default::default()
    };
    input.viewports.insert(
        pin_id(id),
        egui::ViewportInfo {
            inner_rect: Some(rect),
            outer_rect: Some(rect),
            native_pixels_per_point: Some(scale),
            focused: Some(true),
            ..Default::default()
        },
    );
    input
}

fn paint_pin(app: &Snip2Pin, ctx: &egui::Context, input: egui::RawInput) -> egui::FullOutput {
    let viewport = input.viewport_id;
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("production pin callback");
    let mut output = ctx.run_ui(input, |ui| {
        callback(ui);
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx()
                .request_discard("pin input must dispatch once across passes");
        }
    });
    output.textures_delta.clear();
    output
}

fn consume(app: &mut Snip2Pin, ctx: &egui::Context, mut input: egui::RawInput) -> egui::FullOutput {
    input.viewport_id = ViewportId::ROOT;
    input.screen_rect = None;
    input.events.clear();
    let mut output = ctx.run_ui(input, |ui| {
        eframe::App::logic(app, ui.ctx(), &mut eframe::Frame::_new_kittest());
    });
    output.textures_delta.clear();
    output
}

fn geometry(output: &egui::FullOutput, id: u64) -> Vec<&ViewportCommand> {
    output
        .viewport_output
        .get(&pin_id(id))
        .into_iter()
        .flat_map(|viewport| &viewport.commands)
        .filter(|command| {
            matches!(
                command,
                ViewportCommand::InnerSize(_)
                    | ViewportCommand::OuterPosition(_)
                    | ViewportCommand::StartDrag
                    | ViewportCommand::BeginResize(_)
            )
        })
        .collect()
}

#[test]
fn callback_keys_keep_child_observations_when_root_geometry_is_stale() {
    for (scale, zoom) in [(1.0, 1.0), (2.0, 1.25), (1.5, 2.0)] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        ctx.set_zoom_factor(zoom);
        let mut app = tiny_pin_app(&ctx);
        let original = app.pins[&1].pixels.clone();
        let initial = app.pins[&1].initial;
        let rect = egui::Rect::from_min_size(egui::pos2(-80.0, 45.0), egui::vec2(240.0, 160.0));
        let child = paint_pin(
            &app,
            &ctx,
            input(
                1,
                rect,
                scale,
                key_tap(egui::Key::ArrowRight)
                    .into_iter()
                    .chain(key_tap(egui::Key::Equals))
                    .collect(),
            ),
        );
        assert!(geometry(&child, 1).is_empty());
        let stale = egui::Rect::from_min_size(egui::pos2(500.0, 600.0), egui::vec2(900.0, 700.0));
        let root = consume(&mut app, &ctx, input(1, stale, 4.0, vec![]));
        let commands = geometry(&root, 1);
        assert_eq!(commands.len(), 2, "one move and one resize: {commands:?}");
        let step = if cfg!(target_os = "macos") {
            1.0
        } else {
            scale.recip()
        };
        assert!(commands.iter().any(|command| {
            matches!(command, ViewportCommand::OuterPosition(position)
                if (*position - (rect.min + egui::vec2(step / zoom, 0.0))).length() < 0.001)
        }));
        assert!(commands.iter().any(|command| {
            matches!(command, ViewportCommand::InnerSize(size)
                if (*size - rect.size() * 1.01).length() < 0.001)
        }));
        assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
        assert_eq!(app.pins[&1].initial, initial);

        paint_pin(
            &app,
            &ctx,
            input(1, rect, scale, key_tap(egui::Key::Num0).into()),
        );
        let restored = consume(&mut app, &ctx, input(1, stale, 4.0, vec![]));
        let expected = egui::vec2(original.size[0] as f32, original.size[1] as f32) / scale / zoom;
        assert!(matches!(geometry(&restored, 1).as_slice(),
            [ViewportCommand::InnerSize(size)] if (*size - expected).length() < 0.001));
    }
}

#[test]
fn recording_prevents_pin_callback_keys_from_dispatching_window_commands() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.recording.start();
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
    let child = paint_pin(
        &app,
        &ctx,
        input(
            1,
            rect,
            2.0,
            key_tap(egui::Key::ArrowRight)
                .into_iter()
                .chain(key_tap(egui::Key::Equals))
                .chain(key_tap(egui::Key::Escape))
                .collect(),
        ),
    );
    assert!(geometry(&child, 1).is_empty());
    let root = consume(&mut app, &ctx, input(1, rect, 2.0, vec![]));
    assert!(geometry(&root, 1).is_empty());
    assert!(
        app.pins.contains_key(&1),
        "recording Escape cannot close a pin"
    );
    assert!(matches!(app.phase, Phase::Idle));
}

#[test]
fn old_pin_callbacks_observe_recording_start_completion_and_cancel() {
    for cancel in [false, true] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        app.system_locale = Locale::English;
        let previous = app.preferences.binding;
        let _ = home_frame(&mut app, &ctx, vec![]);
        let setup = home_frame(&mut app, &ctx, vec![]);
        let callback = setup.viewport_output[&pin_id(1)]
            .viewport_ui_cb
            .as_ref()
            .expect("callback created before recording")
            .clone();
        let change = setup
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.text() == Locale::English.text(Text::ChangeShortcut) =>
                {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
            .expect("change shortcut control");
        let _ = home_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(change)]);
        for pressed in [true, false] {
            let _ = home_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: change,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert!(
            app.recording.active(),
            "the real home action starts recording"
        );
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
        let paint_old = |keys: Vec<egui::Event>| {
            let mut output = ctx.run_ui(input(1, rect, 2.0, keys), |ui| callback(ui));
            output.textures_delta.clear();
            output
        };
        let keys = || {
            key_tap(egui::Key::ArrowRight)
                .into_iter()
                .chain(key_tap(egui::Key::Equals))
                .chain(key_tap(egui::Key::Escape))
                .collect()
        };
        let output = paint_old(keys());
        assert!(geometry(&output, 1).is_empty());
        let output = consume(&mut app, &ctx, input(1, rect, 2.0, vec![]));
        assert!(geometry(&output, 1).is_empty());
        assert!(app.pins.contains_key(&1));
        let finish = if cancel {
            key_tap(egui::Key::Escape).into()
        } else {
            [true, false]
                .map(|pressed| egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: Some(egui::Key::Q),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                })
                .into()
        };
        let _ = home_frame(&mut app, &ctx, finish);
        assert!(!app.recording.active());
        assert_eq!(app.preferences.binding == previous, cancel);
        let output = paint_old(keys());
        assert!(
            geometry(&output, 1).is_empty(),
            "completion/cancel still owns this root frame"
        );
        assert!(
            app.events.try_recv().is_err(),
            "old callbacks cannot enqueue pin commands"
        );
        let _ = home_frame(&mut app, &ctx, vec![]);
        let _ = paint_old(
            key_tap(egui::Key::ArrowRight)
                .into_iter()
                .chain(key_tap(egui::Key::Equals))
                .collect(),
        );
        let output = consume(&mut app, &ctx, input(1, rect, 2.0, vec![]));
        assert!(
            !geometry(&output, 1).is_empty(),
            "pin keyboard input resumes on the next root frame"
        );
    }
}

#[test]
fn queued_callback_keys_cannot_move_or_resize_a_pin_locked_before_consumption() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    let rect = egui::Rect::from_min_size(egui::pos2(50.0, 60.0), egui::vec2(240.0, 160.0));
    paint_pin(
        &app,
        &ctx,
        input(
            1,
            rect,
            2.0,
            key_tap(egui::Key::ArrowRight)
                .into_iter()
                .chain(key_tap(egui::Key::Equals))
                .collect(),
        ),
    );
    app.toggle_pin_lock(1, &ctx);
    let root = consume(&mut app, &ctx, input(1, rect, 2.0, vec![]));
    assert!(app.pins[&1].locked);
    assert!(geometry(&root, 1).is_empty());

    let pointer = egui::pos2(100.0, 100.0);
    let child = paint_pin(
        &app,
        &ctx,
        input(
            1,
            rect,
            2.0,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
    );
    assert!(geometry(&child, 1).is_empty());
    let root = consume(&mut app, &ctx, input(1, rect, 2.0, vec![]));
    assert!(geometry(&root, 1).is_empty());
}

#[test]
fn callback_copy_button_copies_original_without_starting_a_window_gesture() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    let rect = egui::Rect::from_min_size(egui::pos2(50.0, 60.0), egui::vec2(240.0, 160.0));
    let pin = &app.pins[&1];
    let toolbar = pin::toolbar_geometry(
        rect,
        &pin.screen_areas,
        pin.screen,
        pin::coordinate_ratio(pin.display_scale, 1.0, 1.0),
    )
    .expect("copy button fits observed image");
    assert_eq!(toolbar.count, pin::Control::ALL.len());
    let pointer =
        toolbar.bounds.min - rect.min.to_vec2() + egui::Vec2::splat(pin::METRICS.button / 2.0);
    let mut copies = 0;
    for events in [
        vec![egui::Event::PointerMoved(pointer)],
        vec![egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ] {
        let child = paint_pin(&app, &ctx, input(1, rect, 1.0, events));
        assert!(geometry(&child, 1).is_empty());
        let root = consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
        assert!(geometry(&root, 1).is_empty());
        copies += root
            .platform_output
            .commands
            .iter()
            .filter(|command| {
                matches!(command, egui::OutputCommand::CopyImage(image)
                    if image == app.pins[&1].pixels.as_ref())
            })
            .count();
    }
    assert_eq!(copies, 1);
}

#[test]
fn callback_wheel_continuation_on_another_pin_resizes_only_the_original_target() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    add_second_test_pin(&mut app);
    let first = egui::Rect::from_min_size(egui::pos2(50.0, 60.0), egui::vec2(240.0, 160.0));
    let second = egui::Rect::from_min_size(egui::pos2(500.0, 60.0), egui::vec2(360.0, 200.0));
    let wheel = |phase, delta| egui::Event::MouseWheel {
        source: egui::MouseWheelSource::Unknown,
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, delta),
        modifiers: egui::Modifiers::NONE,
        phase,
    };
    paint_pin(
        &app,
        &ctx,
        input(
            1,
            first,
            2.0,
            vec![
                egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
                wheel(egui::TouchPhase::Start, 0.0),
            ],
        ),
    );
    let mut continued = input(
        2,
        second,
        1.0,
        vec![
            egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
            wheel(egui::TouchPhase::Move, 1.0),
        ],
    );
    continued.viewports.insert(
        pin_id(1),
        input(1, first, 2.0, vec![]).viewports[&pin_id(1)].clone(),
    );
    let child = paint_pin(&app, &ctx, continued.clone());
    assert!(geometry(&child, 1).is_empty());
    assert!(geometry(&child, 2).is_empty());
    let root = consume(&mut app, &ctx, continued);
    let commands = geometry(&root, 1);
    assert_eq!(commands.len(), 2, "one anchored resize: {commands:?}");
    assert!(commands.iter().any(|command| {
        matches!(command, ViewportCommand::InnerSize(size)
            if size.x > first.width() && size.y > first.height())
    }));
    assert!(geometry(&root, 2).is_empty());
}
