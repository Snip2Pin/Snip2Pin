use super::*;

fn sandbox(ctx: &egui::Context) -> Sandbox {
    let mut sandbox = Sandbox::new(
        ctx,
        Vec::new(),
        ColorImage::filled([2, 2], egui::Color32::LIGHT_BLUE),
    );
    for id in 1..=2 {
        sandbox.pins.push(Pin {
            id,
            name: String::new(),
            texture: ctx.load_texture(
                format!("test pin {id}"),
                ColorImage::filled([32, 24], egui::Color32::WHITE),
                Default::default(),
            ),
            image: Rect::from_min_size(
                egui::pos2(700.0, id as f32 * 150.0),
                egui::vec2(120.0, 80.0),
            ),
            gesture: None,
            locked: false,
        });
    }
    sandbox.next_id = 2;
    sandbox
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn frame(ctx: &egui::Context, sandbox: &mut Sandbox, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| sandbox.show(ui),
    );
    output.textures_delta.clear();
    output
}

#[test]
fn escape_only_closes_the_focused_pin() {
    for focused in [None, Some(1), Some(2)] {
        let ctx = egui::Context::default();
        let mut sandbox = sandbox(&ctx);
        let _ = frame(&ctx, &mut sandbox, vec![]);
        sandbox.focused_pin = focused;
        let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::Escape)]);
        let remaining: Vec<_> = sandbox.pins.iter().map(|pin| pin.id).collect();
        assert_eq!(
            remaining,
            (1..=2)
                .filter(|id| Some(*id) != focused)
                .collect::<Vec<_>>(),
            "browser pins must obey the same focus rule as native windows"
        );
    }
}

#[test]
fn menu_and_recording_own_input_until_the_end_of_the_frame() {
    for recording in [false, true] {
        let ctx = egui::Context::default();
        let mut sandbox = sandbox(&ctx);
        let _ = frame(&ctx, &mut sandbox, vec![]);
        sandbox.focused_pin = Some(1);
        if recording {
            sandbox.recording.start();
        }
        if !recording {
            sandbox.menu = Some((1, egui::pos2(600.0, 400.0)));
        }
        let images: Vec<_> = sandbox.pins.iter().map(|pin| pin.image).collect();
        let _ = frame(
            &ctx,
            &mut sandbox,
            vec![key(egui::Key::ArrowRight), key(egui::Key::Plus)],
        );
        assert_eq!(
            sandbox.pins.iter().map(|pin| pin.image).collect::<Vec<_>>(),
            images
        );
        let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::Escape)]);
        assert_eq!(
            sandbox.pins.len(),
            2,
            "Escape dismisses the active interaction"
        );
        assert!(!sandbox.recording.active() && sandbox.menu.is_none());
        let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::Escape)]);
        assert_eq!(
            sandbox.pins.iter().map(|pin| pin.id).collect::<Vec<_>>(),
            [2]
        );
    }
}

#[test]
fn shortcut_recording_uses_physical_keys_and_preserves_the_old_binding_on_errors() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let _ = frame(&ctx, &mut sandbox, vec![]);
    let old = sandbox.preferences.binding;
    sandbox.recording.start();
    let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::A)]);
    assert!(sandbox.recording.active());
    assert_eq!(sandbox.preferences.binding, old);
    assert_eq!(
        sandbox.recording.feedback(),
        Some(&Text::ShortcutModifiers.into())
    );
    let modifiers = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
    let event = egui::Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::Q),
        pressed: true,
        repeat: false,
        modifiers,
    };
    let mut released = key(egui::Key::A);
    if let egui::Event::Key { pressed, .. } = &mut released {
        *pressed = false;
    }
    let _ = frame(&ctx, &mut sandbox, vec![released, event.clone()]);
    assert!(!sandbox.recording.active() && sandbox.message.is_none());
    assert!(sandbox.preferences.binding.matches(&event));
    assert!(
        sandbox.capture_gate.active().is_none(),
        "recording cannot capture"
    );
}

#[test]
fn recording_the_current_shortcut_confirms_success_without_capture() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let _ = frame(&ctx, &mut sandbox, vec![]);
    let original = sandbox.preferences;
    sandbox.recording.start();
    let _ = frame(
        &ctx,
        &mut sandbox,
        vec![egui::Event::Key {
            key: egui::Key::Num2,
            physical_key: Some(egui::Key::Num2),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT
                | if cfg!(target_os = "macos") {
                    egui::Modifiers::MAC_CMD
                } else {
                    egui::Modifiers::CTRL
                },
        }],
    );
    assert!(!sandbox.recording.active());
    assert_eq!(sandbox.preferences, original);
    assert_eq!(
        sandbox.recording.feedback(),
        Some(&Text::ShortcutConfirmed.into())
    );
    assert!(!sandbox.recording.feedback_is_error());
    assert!(sandbox.capture_gate.active().is_none());
}

#[test]
fn modifier_leading_recording_keeps_the_saved_binding_until_a_complete_combo() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let _ = frame(&ctx, &mut sandbox, vec![]);
    let old = sandbox.preferences.binding;
    sandbox.recording.start();
    let _ = frame(
        &ctx,
        &mut sandbox,
        vec![egui::Event::Key {
            key: egui::Key::ControlLeft,
            physical_key: Some(egui::Key::ControlLeft),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        }],
    );
    assert!(
        sandbox.recording.active(),
        "holding Ctrl must keep recording"
    );
    assert_eq!(sandbox.preferences.binding, old);
    assert!(
        sandbox.message.is_none(),
        "an unfinished chord is not an error"
    );
}

#[test]
fn recording_a_shortcut_cannot_start_capture_on_a_repeated_layout_pass() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let _ = frame(&ctx, &mut sandbox, vec![]);
    sandbox.recording.start();
    let mut event = key(egui::Key::Q);
    if let egui::Event::Key { modifiers, .. } = &mut event {
        *modifiers = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
    }
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(sandbox.desktop_bounds),
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            sandbox.show(ui);
            if ui.ctx().current_pass_index() == 0 {
                ui.ctx()
                    .request_discard("exercise repeated shortcut layout");
            }
        },
    );
    output.textures_delta.clear();
    assert_eq!(output.platform_output.num_completed_passes, 2);
    assert!(!sandbox.recording.active());
    assert!(
        sandbox.capture_gate.active().is_none(),
        "recording is not a capture gesture"
    );
}

#[test]
fn cancelled_capture_cannot_accept_a_late_canvas_image_even_in_the_same_frame() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let _ = frame(&ctx, &mut sandbox, vec![]);
    sandbox.capture(&ctx);
    let old = sandbox.capture_gate.active().expect("pending request");
    let old_callback = screenshot_callback(frame(&ctx, &mut sandbox, vec![]));
    sandbox.capture(&ctx);
    assert!(sandbox.capture_gate.active().is_none());
    sandbox.capture(&ctx);
    let current = sandbox.capture_gate.active().expect("new request");
    let current_callback = screenshot_callback(frame(&ctx, &mut sandbox, vec![]));
    assert_ne!(old, current);
    let image = || Arc::new(ColorImage::filled([100, 80], egui::Color32::WHITE));
    let abandoned = image();
    let weak = Arc::downgrade(&abandoned);
    old_callback.complete(abandoned);
    assert!(
        weak.upgrade().is_none(),
        "cancelled callbacks release their pixels"
    );
    let _ = frame(&ctx, &mut sandbox, vec![]);
    assert!(sandbox.selection.is_none());
    assert_eq!(sandbox.capture_gate.active(), Some(current));
    current_callback.clone().complete(image());
    let _ = frame(&ctx, &mut sandbox, vec![]);
    assert!(sandbox.selection.is_some());
    assert!(sandbox.capture_gate.active().is_none());
    let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::Escape)]);
    current_callback.complete(image());
    let _ = frame(&ctx, &mut sandbox, vec![]);
    assert!(
        sandbox.selection.is_none(),
        "duplicate results cannot reopen capture"
    );
    assert_eq!(sandbox.pins.len(), 2);
}

fn screenshot_callback(mut output: egui::FullOutput) -> egui::ScreenshotCallback {
    output.textures_delta.clear();
    let mut callbacks = output
        .viewport_output
        .into_values()
        .flat_map(|viewport| viewport.commands)
        .filter_map(|command| {
            if let ViewportCommand::Screenshot(callback) = command {
                Some(callback)
            } else {
                None
            }
        });
    let callback = callbacks.next().expect("canvas screenshot callback");
    assert!(
        callbacks.next().is_none(),
        "one callback per capture request"
    );
    callback
}

#[test]
fn canvas_callback_survives_repeated_layout_and_escape_discards_ready_pixels() {
    for cancel in [false, true] {
        let ctx = egui::Context::default();
        let mut sandbox = sandbox(&ctx);
        let _ = frame(&ctx, &mut sandbox, vec![]);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| {
                if ui.ctx().current_pass_index() == 0 {
                    sandbox.capture(ui.ctx());
                    ui.ctx().request_discard("repeat the capture-start layout");
                }
                sandbox.show(ui);
            },
        );
        assert_eq!(output.platform_output.num_completed_passes, 2);
        let callback = screenshot_callback(output);
        let pixels = Arc::new(ColorImage::filled([100, 80], egui::Color32::WHITE));
        let weak = Arc::downgrade(&pixels);
        callback.complete(pixels);
        let _ = frame(
            &ctx,
            &mut sandbox,
            if cancel {
                vec![key(egui::Key::Escape)]
            } else {
                vec![]
            },
        );
        assert_eq!(sandbox.selection.is_some(), !cancel);
        assert_eq!(weak.upgrade().is_some(), !cancel);
        assert!(sandbox.capture_gate.active().is_none());
        let _ = frame(&ctx, &mut sandbox, vec![]);
        assert_eq!(sandbox.selection.is_some(), !cancel);
        assert_eq!(sandbox.pins.len(), 2);
    }
}

#[test]
fn frozen_pixels_are_released_on_cancel_or_pin_creation_and_placement_matches_the_preview() {
    for cancel in [false, true] {
        let ctx = egui::Context::default();
        let mut sandbox = sandbox(&ctx);
        let _ = frame(&ctx, &mut sandbox, vec![]);
        sandbox.capture(&ctx);
        let callback = screenshot_callback(frame(&ctx, &mut sandbox, vec![]));
        let source = Arc::new(ColorImage::filled([200, 160], egui::Color32::WHITE));
        let weak = Arc::downgrade(&source);
        callback.complete(source);
        let _ = frame(&ctx, &mut sandbox, vec![]);
        assert!(sandbox.selection.is_some() && weak.upgrade().is_some());
        if cancel {
            let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::Escape)]);
        } else {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let start = egui::pos2(940.0, 740.0);
            let end = egui::pos2(900.0, 700.0);
            let _ = frame(
                &ctx,
                &mut sandbox,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let _ = frame(&ctx, &mut sandbox, vec![egui::Event::PointerMoved(end)]);
            let _ = frame(&ctx, &mut sandbox, vec![button(end, false)]);
            let pin = sandbox.pins.last().expect("new pin");
            assert_eq!(pin.image, Rect::from_min_size(end, egui::vec2(40.0, 40.0)));
            assert_eq!(
                pin.texture.size(),
                [8, 8],
                "display units cannot overwrite source pixels"
            );
        }
        assert!(sandbox.selection.is_none());
        let _ = frame(&ctx, &mut sandbox, vec![]);
        assert!(
            weak.upgrade().is_none(),
            "finished captures cannot retain full desktop pixels"
        );
        assert_eq!(sandbox.pins.len(), if cancel { 2 } else { 3 });
    }
}

#[test]
fn system_theme_updates_the_auto_icon_without_changing_preferences_or_pins() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let preferences = sandbox.preferences;
    for (theme, variant) in [
        (egui::Theme::Dark, snip2pin::icon::Variant::Metal),
        (egui::Theme::Light, snip2pin::icon::Variant::Line),
    ] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                system_theme: Some(theme),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| sandbox.show(ui),
        );
        output.textures_delta.clear();
        assert_eq!(sandbox.applied_icon, variant);
        assert_eq!(sandbox.preferences, preferences);
        assert_eq!(sandbox.pins.len(), 2);
    }
}

#[test]
fn locks_guard_stale_actions_and_closing_releases_shared_ui_state() {
    let ctx = egui::Context::default();
    let mut sandbox = sandbox(&ctx);
    let _ = frame(&ctx, &mut sandbox, vec![]);
    sandbox.focused_pin = Some(1);
    let _ = frame(&ctx, &mut sandbox, vec![key(egui::Key::ArrowRight)]);
    let original = sandbox.pins[0].image;
    let original_texture = sandbox.pins[0].texture.id();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let bounds = ui.max_rect();
        sandbox.pin_action(pin_view::Action::ToggleLock(1), ui, bounds);
        for action in [
            pin_view::Action::ClosePin(1),
            pin_view::Action::Zoom(1, vec![pin::Zoom::In]),
            pin_view::Action::Move(1, [50, 50]),
        ] {
            sandbox.pin_action(action, ui, bounds);
        }
        assert_eq!(sandbox.pins.len(), 2);
        assert_eq!(sandbox.pins[0].image, original);
        assert_eq!(sandbox.pins[0].texture.id(), original_texture);
        sandbox.pin_action(pin_view::Action::UnlockAll, ui, bounds);
    });
    output.textures_delta.clear();
    let mut repeated = key(egui::Key::ArrowRight);
    if let egui::Event::Key { repeat, .. } = &mut repeated {
        *repeat = true;
    }
    let _ = frame(&ctx, &mut sandbox, vec![repeated]);
    assert_eq!(
        sandbox.pins[0].image, original,
        "locking interrupted the old key hold"
    );
    sandbox.menu = Some((1, egui::pos2(600.0, 400.0)));
    sandbox.close_pin(1, &ctx);
    assert!(sandbox.focused_pin.is_none() && sandbox.menu.is_none());
    assert_eq!(
        sandbox.pins.iter().map(|pin| pin.id).collect::<Vec<_>>(),
        [2]
    );
    sandbox.message = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        sandbox.pin_action(pin_view::Action::CopyPin(1), ui, ui.max_rect());
        assert!(
            sandbox.message.is_none(),
            "closed pins cannot report a copy"
        );
        sandbox.pin_action(pin_view::Action::CopyPin(2), ui, ui.max_rect());
        assert_eq!(sandbox.message, Some(Text::DemoCopied.into()));
    });
    output.textures_delta.clear();
}
