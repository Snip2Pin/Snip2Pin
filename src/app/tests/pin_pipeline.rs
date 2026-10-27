use super::*;

#[test]
fn small_locked_close_press_reaches_the_screen_notice_presentation() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.toggle_pin_lock(1, &ctx);
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(26.0, 26.0));
    let point = egui::pos2(13.0, 13.0);
    paint_pin(
        &app,
        &ctx,
        input(1, rect, 1.0, vec![egui::Event::PointerMoved(point)]),
    );
    paint_pin(
        &app,
        &ctx,
        input(
            1,
            rect,
            1.0,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
    );
    consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    assert!(app.layer_presentation.is_some());
    assert!(app.pins[&1].locked);
    assert!(matches!(app.phase, Phase::Idle));
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&layer_id(0)]
        .viewport_ui_cb
        .as_ref()
        .unwrap();
    let mut layer_input = egui::RawInput {
        viewport_id: layer_id(0),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            app.layers[&0].display.logical_size().into(),
        )),
        ..Default::default()
    };
    layer_input
        .viewports
        .insert(layer_id(0), egui::ViewportInfo::default());
    let mut output = ctx.run_ui(layer_input, |ui| callback(ui));
    output.textures_delta.clear();
    let text = ui::screen_hint::unlock_text(&ctx);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text_shape) if text_shape.galley.text() == text)));
    assert!(
        output
            .viewport_output
            .values()
            .flat_map(|viewport| &viewport.commands)
            .all(|command| !matches!(
                command,
                ViewportCommand::Focus | ViewportCommand::MousePassthrough(false)
            ))
    );
}

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
fn unfocused_popup_escape_delivered_to_the_pin_closes_only_the_menu() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    let original = app.pins[&1].pixels.clone();
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback created before opening the menu")
        .clone();
    let point = egui::pos2(100.0, 100.0);
    for events in [
        vec![egui::Event::PointerMoved(point)],
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ] {
        let mut output = ctx.run_ui(input(1, rect, 1.0, events), |ui| callback(ui));
        output.textures_delta.clear();
        consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    }
    assert!(
        app.pin_menu.is_some(),
        "the actual right click opens a menu"
    );
    let mut keys = input(1, rect, 1.0, key_tap(egui::Key::Escape).into());
    keys.viewports.insert(
        pin_menu_id(),
        egui::ViewportInfo {
            focused: Some(false),
            ..Default::default()
        },
    );
    let mut output = ctx.run_ui(keys, |ui| {
        callback(ui);
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx().request_discard("menu dismissal owns every pass");
        }
    });
    output.textures_delta.clear();
    let root = consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    assert!(
        app.pins.contains_key(&1),
        "menu Escape must not close its pin"
    );
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    assert!(app.pin_menu.is_none());
    assert!(geometry(&root, 1).is_empty());
    let mut output = ctx.run_ui(input(1, rect, 1.0, vec![]), |ui| callback(ui));
    output.textures_delta.clear();
    assert!(
        app.events.try_recv().is_err(),
        "an existing callback must not replay the dismissed input"
    );
    consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    paint_pin(
        &app,
        &ctx,
        input(1, rect, 1.0, key_tap(egui::Key::ArrowRight).into()),
    );
    let root = consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    assert!(
        !geometry(&root, 1).is_empty(),
        "fresh input works after dismissal"
    );
}

#[test]
fn popup_owns_keys_scroll_and_the_outside_click_on_another_pin() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    add_second_test_pin(&mut app);
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
    app.open_pin_menu(1, None, &ctx);
    for target in [1, 2] {
        let mut events: Vec<_> = key_tap(egui::Key::ArrowRight)
            .into_iter()
            .chain(key_tap(egui::Key::Equals))
            .chain(key_tap(egui::Key::Num0))
            .collect();
        events.extend([
            egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
            egui::Event::MouseWheel {
                source: egui::MouseWheelSource::Unknown,
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, 1.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Start,
            },
            egui::Event::Zoom(1.25),
        ]);
        let child = paint_pin(&app, &ctx, input(target, rect, 2.0, events));
        let root = consume(&mut app, &ctx, input(target, rect, 2.0, vec![]));
        for id in [1, 2] {
            assert!(geometry(&child, id).is_empty());
            assert!(geometry(&root, id).is_empty());
        }
        assert_eq!(app.pins.len(), 2);
        assert!(app.pin_menu.is_some());
    }
    let toolbar = pin::toolbar_geometry(rect, &[], rect, 1.0).expect("toolbar");
    let copy =
        toolbar.bounds.min - rect.min.to_vec2() + egui::Vec2::splat(pin::METRICS.button / 2.0);
    for events in [
        vec![egui::Event::PointerMoved(copy)],
        vec![egui::Event::PointerButton {
            pos: copy,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerButton {
            pos: copy,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ] {
        let child = paint_pin(&app, &ctx, input(2, rect, 1.0, events));
        let root = consume(&mut app, &ctx, input(2, rect, 1.0, vec![]));
        assert!(geometry(&child, 2).is_empty());
        assert!(geometry(&root, 2).is_empty());
        assert!(
            root.platform_output.commands.is_empty(),
            "dismissal must not copy"
        );
    }
    assert!(app.pin_menu.is_none());
    assert_eq!(app.pins.len(), 2);
}

#[test]
fn pin_blur_before_the_menu_focus_callback_does_not_dismiss_the_menu() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.open_pin_menu(1, None, &ctx);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.show_pins(ui.ctx());
        app.show_pin_menu(ui.ctx());
    });
    setup.textures_delta.clear();
    let pin = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    let menu = setup.viewport_output[&pin_menu_id()]
        .viewport_ui_cb
        .as_ref()
        .expect("menu callback");
    let rect = app.pins[&1].initial;
    let mut blur = input(1, rect, 1.0, vec![egui::Event::WindowFocused(false)]);
    blur.focused = false;
    blur.viewports.get_mut(&pin_id(1)).expect("pin").focused = Some(false);
    blur.viewports.insert(
        pin_menu_id(),
        egui::ViewportInfo {
            focused: Some(false),
            ..Default::default()
        },
    );
    let mut output = ctx.run_ui(blur, |ui| pin(ui));
    output.textures_delta.clear();
    consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    assert!(
        app.pin_menu.is_some(),
        "a stale menu focus cache must not dismiss a menu during focus transfer"
    );
    let mut gained = egui::RawInput {
        viewport_id: pin_menu_id(),
        events: vec![egui::Event::WindowFocused(true)],
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            ui::pin::MENU_SIZE,
        )),
        ..Default::default()
    };
    gained.viewports.insert(
        pin_menu_id(),
        egui::ViewportInfo {
            focused: Some(true),
            ..Default::default()
        },
    );
    let mut output = ctx.run_ui(gained, |ui| menu(ui));
    output.textures_delta.clear();
    consume(&mut app, &ctx, input(1, rect, 1.0, vec![]));
    assert!(app.pin_menu.is_some() && app.pins.contains_key(&1));
}

#[test]
fn focus_loss_dismisses_a_popup_only_with_authoritative_external_focus() {
    for current_focus in [Some(true), Some(false), None] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        app.open_pin_menu(1, None, &ctx);
        let rect = app.pins[&1].initial;
        let mut blur = input(1, rect, 1.0, vec![egui::Event::WindowFocused(false)]);
        blur.focused = false;
        blur.viewports.get_mut(&pin_id(1)).expect("pin").focused = Some(false);
        blur.viewports.insert(
            pin_menu_id(),
            egui::ViewportInfo {
                focused: Some(false),
                ..Default::default()
            },
        );
        paint_pin(&app, &ctx, blur);
        let events: Vec<_> = app.events.try_iter().collect();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::PinMenuFocusLost))
                .count(),
            1,
            "a blur requests one authoritative focus check"
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::ClosePinMenu))
        );
        let mut checks = 0;
        let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            app.check_pin_menu_focus(ctx, |_| {
                checks += 1;
                current_focus
            });
        });
        assert_eq!(checks, 1);
        assert_eq!(app.pin_menu.is_some(), current_focus != Some(false));
        assert!(app.pins.contains_key(&1));
    }
}

#[test]
fn menu_dismissal_releases_pin_input_while_home_only_runs_logic() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.home_visible = false;
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback")
        .clone();
    app.open_pin_menu(1, None, &ctx);
    let mut output = ctx.run_ui(
        input(1, rect, 1.0, key_tap(egui::Key::Escape).into()),
        |ui| callback(ui),
    );
    output.textures_delta.clear();
    let root_frame = ctx.cumulative_frame_nr_for(ViewportId::ROOT);
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert!(app.pin_menu.is_none() && app.pins.contains_key(&1));
    let mut output = ctx.run_ui(
        input(1, rect, 1.0, key_tap(egui::Key::ArrowRight).into()),
        |ui| callback(ui),
    );
    output.textures_delta.clear();
    let root = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert_eq!(ctx.cumulative_frame_nr_for(ViewportId::ROOT), root_frame);
    assert!(
        root.viewport_commands
            .get(&pin_id(1))
            .is_some_and(|commands| {
                commands
                    .iter()
                    .any(|command| matches!(command, ViewportCommand::OuterPosition(_)))
            }),
        "a hidden Home must not retain menu ownership over fresh pin input"
    );
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
fn hiding_home_dismisses_recording_and_releases_existing_pin_callbacks() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    let previous = app.preferences.binding;
    app.recording.start();
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("callback made while recording")
        .clone();
    assert!(ui::shortcut::Recorder::input_blocked(&ctx));
    let hidden = ctx.run_logic(&egui::RawInput::default(), |ctx| app.hide_home(ctx));
    assert!(!app.home_visible && !app.exiting);
    assert!(!app.recording.active());
    assert!(!ui::shortcut::Recorder::input_blocked(&ctx));
    assert_eq!(app.preferences.binding, previous);
    assert!(app.menu_state().capture_enabled);
    assert!(hidden.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Visible(false)));
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
    let mut output = ctx.run_ui(
        input(1, rect, 2.0, key_tap(egui::Key::ArrowRight).into()),
        |ui| callback(ui),
    );
    output.textures_delta.clear();
    let output = consume(&mut app, &ctx, input(1, rect, 2.0, vec![]));
    assert!(
        !geometry(&output, 1).is_empty(),
        "pins work without reopening hidden home"
    );
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| app.capture_command(ctx));
    assert!(
        matches!(app.phase, Phase::Capturing),
        "generated demo capture works after hiding home"
    );
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
