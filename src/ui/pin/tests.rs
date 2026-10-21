use std::cell::RefCell;

use super::*;
use egui::{Event, FullOutput, Modifiers, PointerButton, Vec2};

mod gestures;
mod keyboard;

fn image(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    native_edges: bool,
    excluded: Option<Rect>,
    events: &dyn Fn(Action),
) -> bool {
    let ctx = ui.ctx().clone();
    image_with_host(
        ui,
        id,
        texture,
        Interaction {
            native_edges,
            toolbar: excluded.map(|bounds| Toolbar { bounds, count: 0 }),
            ..Default::default()
        },
        events,
        |command| ctx.send_viewport_cmd(command),
    )
}

fn controls(ui: &mut egui::Ui, id: u64, events: &dyn Fn(Action)) {
    let bounds = ui.max_rect();
    let toolbar = snip2pin::pin::toolbar_geometry(bounds, &[], bounds, 1.0)
        .expect("test controls fit their bounds");
    super::controls(ui, id, false, toolbar, events);
}

fn pointer(point: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn complete_view_owns_focus_painting_and_excludes_buttons_from_native_gestures() {
    for focused in [false, true] {
        for native_edges in [false, true] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            let toolbar = snip2pin::pin::toolbar_geometry(bounds, &[], bounds, 1.0)
                .expect("room for all controls");
            let point = toolbar.bounds.min + Vec2::splat(METRICS.button / 2.0);
            let actions = RefCell::new(Vec::new());
            let host = RefCell::new(Vec::new());
            for events in [
                vec![Event::PointerMoved(point)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
            ] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        show(
                            ui,
                            7,
                            &texture,
                            Interaction {
                                native_edges,
                                toolbar: Some(toolbar),
                                keyboard_focus: Some(focused),
                                ..Default::default()
                            },
                            &|action| actions.borrow_mut().push(action),
                            |command| host.borrow_mut().push(command),
                        );
                    },
                );
                output.textures_delta.clear();
                let highlights = output.shapes.iter().filter(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == PIN.highlight)
                });
                assert_eq!(highlights.count(), usize::from(focused));
                assert_eq!(image_mesh(&output, &texture), bounds);
            }
            assert_eq!(actions.into_inner(), [Action::CopyPin(7)]);
            assert!(host.into_inner().is_empty());
        }
    }
}

#[test]
fn locked_pin_hint_is_not_painted_inside_the_image_viewport() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(160.0, 80.0));
    let actions = RefCell::new(Vec::new());
    for frame in 0..3 {
        let point = bounds.center();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                events: if frame == 1 {
                    vec![Event::PointerMoved(point), pointer(point, true)]
                } else {
                    vec![Event::PointerMoved(point)]
                },
                time: Some(frame as f64),
                ..Default::default()
            },
            |ui| {
                image_with_host(
                    ui,
                    7,
                    &texture,
                    Interaction {
                        locked: true,
                        ..Default::default()
                    },
                    &|action| actions.borrow_mut().push(action),
                    |_| {},
                );
            },
        );
        output.textures_delta.clear();
        assert!(
            output
                .shapes
                .iter()
                .all(|shape| !matches!(&shape.shape, egui::Shape::Text(_))),
            "a screen-level recovery hint must not be clipped inside a tiny image window"
        );
    }
    assert_eq!(*actions.borrow(), [Action::UnlockHint(7)]);
}

fn texture(ctx: &egui::Context) -> TextureHandle {
    ctx.load_texture(
        "generated-pin",
        egui::ColorImage::filled([2, 2], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    )
}

#[test]
fn recovery_shortcut_unlocks_a_locked_unfocused_image_once_without_geometry_changes() {
    let ctx = egui::Context::default();
    let image = texture(&ctx);
    let events = RefCell::new(Vec::new());
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
        events: vec![Event::Key {
            key: egui::Key::L,
            physical_key: Some(egui::Key::L),
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                alt: true,
                shift: true,
                ctrl: !cfg!(target_os = "macos"),
                mac_cmd: cfg!(target_os = "macos"),
                command: true,
            },
        }],
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        image_with_host(
            ui,
            42,
            &image,
            Interaction {
                locked: true,
                keyboard_focus: Some(false),
                ..Default::default()
            },
            &|action| events.borrow_mut().push(action),
            |_| panic!("recovery must not drag or resize"),
        );
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx().request_discard("recovery multipass");
        }
    });
    assert_eq!(*events.borrow(), [Action::UnlockAll]);
    assert_no_geometry(&output);
    output.textures_delta.clear();
}

fn commands(output: &FullOutput) -> &[ViewportCommand] {
    &output.viewport_output[&egui::ViewportId::ROOT].commands
}

fn assert_no_geometry(output: &FullOutput) {
    assert!(
        commands(output).iter().all(|command| !matches!(
            command,
            ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
        )),
        "the view delegates gestures and never corrects native geometry"
    );
}

fn paint_image(
    ctx: &egui::Context,
    texture: &TextureHandle,
    bounds: Rect,
    events: Vec<Event>,
    native_edges: bool,
    time: f64,
    multipass: bool,
) -> (FullOutput, Vec<Action>) {
    let actions = RefCell::new(Vec::new());
    let mut input = egui::RawInput {
        screen_rect: Some(bounds),
        events,
        time: Some(time),
        ..Default::default()
    };
    // A viewport's desktop position is irrelevant to local image painting.
    // Deliberately keep this different from the observed content size.
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .inner_rect = Some(Rect::from_min_size(
        egui::pos2(700.0, 500.0),
        egui::vec2(90.0, 60.0),
    ));
    let mut output = ctx.run_ui(input, |ui| {
        image(ui, 7, texture, native_edges, None, &|action| {
            actions.borrow_mut().push(action)
        });
        if multipass && ui.ctx().current_pass_index() == 0 {
            ui.ctx()
                .request_discard("exercise repeated native layout pass");
        }
    });
    output.textures_delta.clear();
    assert_no_geometry(&output);
    (output, actions.into_inner())
}

fn image_mesh(output: &FullOutput, texture: &TextureHandle) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                let bounds = mesh.calc_bounds();
                assert!(shape.clip_rect.contains_rect(bounds));
                Some(bounds)
            }
            _ => None,
        })
        .expect("image mesh")
}

#[test]
fn image_follows_observed_bounds_even_after_discarded_passes() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    for (index, size) in [
        egui::vec2(240.0, 160.0),
        egui::vec2(560.0, 90.0),
        egui::vec2(32.0, 380.0),
        egui::vec2(3.0, 3.0),
    ]
    .into_iter()
    .enumerate()
    {
        let bounds = Rect::from_min_size(Pos2::ZERO, size);
        let (output, actions) =
            paint_image(&ctx, &texture, bounds, vec![], true, index as f64, true);
        assert_eq!(output.platform_output.num_completed_passes, 2);
        assert_eq!(image_mesh(&output, &texture), bounds);
        assert!(actions.is_empty());
    }
}

#[test]
fn body_press_requests_system_drag_once_without_position_updates() {
    for native_edges in [false, true] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
        let press = bounds.center();
        paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::PointerMoved(press)],
            native_edges,
            0.0,
            false,
        );
        let (output, _) = paint_image(
            &ctx,
            &texture,
            bounds,
            vec![pointer(press, true)],
            native_edges,
            0.1,
            true,
        );
        assert_eq!(commands(&output), &[ViewportCommand::StartDrag]);
        let (output, _) = paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::PointerMoved(press + egui::vec2(40.0, 20.0))],
            native_edges,
            0.2,
            false,
        );
        assert!(commands(&output).is_empty());
        assert_eq!(image_mesh(&output, &texture), bounds);
    }
}

#[test]
fn non_native_edges_request_system_resize_on_press_in_all_directions() {
    use ResizeDirection::*;
    for (point, direction) in [
        (egui::pos2(1.0, 1.0), NorthWest),
        (egui::pos2(120.0, 1.0), North),
        (egui::pos2(239.0, 1.0), NorthEast),
        (egui::pos2(1.0, 80.0), West),
        (egui::pos2(239.0, 80.0), East),
        (egui::pos2(1.0, 159.0), SouthWest),
        (egui::pos2(120.0, 159.0), South),
        (egui::pos2(239.0, 159.0), SouthEast),
    ] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
        paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::PointerMoved(point)],
            false,
            0.0,
            false,
        );
        let (output, _) = paint_image(
            &ctx,
            &texture,
            bounds,
            vec![pointer(point, true)],
            false,
            0.1,
            true,
        );
        assert_eq!(
            commands(&output),
            &[ViewportCommand::BeginResize(direction)]
        );
    }
}

#[test]
fn appkit_edges_never_dispatch_emulated_resize() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
    let point = bounds.min + Vec2::splat(1.0);
    paint_image(
        &ctx,
        &texture,
        bounds,
        vec![Event::PointerMoved(point)],
        true,
        0.0,
        false,
    );
    let (output, _) = paint_image(
        &ctx,
        &texture,
        bounds,
        vec![pointer(point, true)],
        true,
        0.1,
        false,
    );
    assert!(
        !commands(&output)
            .iter()
            .any(|command| matches!(command, ViewportCommand::BeginResize(_)))
    );
}

#[test]
fn batched_pointer_motion_keeps_the_pressed_resize_edge() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
    let press = egui::pos2(1.0, 80.0);
    paint_image(
        &ctx,
        &texture,
        bounds,
        vec![Event::PointerMoved(press)],
        false,
        0.0,
        false,
    );
    let (output, _) = paint_image(
        &ctx,
        &texture,
        bounds,
        vec![pointer(press, true), Event::PointerMoved(bounds.center())],
        false,
        0.1,
        false,
    );
    assert_eq!(
        commands(&output),
        &[ViewportCommand::BeginResize(ResizeDirection::West)]
    );
}

#[test]
fn image_double_clicks_do_not_close_but_escape_does() {
    for native_edges in [false, true] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
        let point = bounds.center();
        paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::PointerMoved(point)],
            native_edges,
            0.0,
            false,
        );
        for (index, pressed) in [true, false, true, false, true, false]
            .into_iter()
            .enumerate()
        {
            let (_, actions) = paint_image(
                &ctx,
                &texture,
                bounds,
                vec![pointer(point, pressed)],
                native_edges,
                (index + 1) as f64 * 0.05,
                true,
            );
            assert!(
                actions.is_empty(),
                "image clicks must never close a pin: {actions:?}"
            );
        }
        let (_, actions) = paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            native_edges,
            1.0,
            true,
        );
        assert_eq!(actions, [Action::ClosePin(7)]);
    }
}

#[test]
fn controls_copy_lock_and_close_without_touching_window_geometry() {
    for (offset, expected) in [
        (0.0, Action::CopyPin(7)),
        (METRICS.button + METRICS.button_gap, Action::ToggleLock(7)),
        (
            2.0 * (METRICS.button + METRICS.button_gap),
            Action::ClosePin(7),
        ),
    ] {
        let ctx = egui::Context::default();
        let size = METRICS.toolbar_size();
        let point = egui::pos2(offset + METRICS.button / 2.0, METRICS.button / 2.0);
        let actions = RefCell::new(Vec::new());
        for events in [
            vec![Event::PointerMoved(point)],
            vec![pointer(point, true)],
            vec![pointer(point, false)],
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |ui| controls(ui, 7, &|action| actions.borrow_mut().push(action)),
            );
            output.textures_delta.clear();
            assert_no_geometry(&output);
        }
        assert_eq!(actions.into_inner(), [expected]);
    }
}

fn paint_composite(
    ctx: &egui::Context,
    texture: &TextureHandle,
    bounds: Rect,
    row: Rect,
    events: Vec<Event>,
    native_edges: bool,
) -> (FullOutput, Vec<Action>, bool) {
    let actions = RefCell::new(Vec::new());
    let mut visible = false;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(bounds),
            events,
            ..Default::default()
        },
        |ui| {
            let controls_rect = ctx
                .pointer_latest_pos()
                .filter(|point| ui.max_rect().contains(*point))
                .map(|_| row);
            visible = controls_rect.is_some();
            image(ui, 7, texture, native_edges, Some(row), &|action| {
                actions.borrow_mut().push(action)
            });
            if let Some(row) = controls_rect {
                ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
                    controls(ui, 7, &|action| actions.borrow_mut().push(action));
                });
            }
        },
    );
    output.textures_delta.clear();
    assert_no_geometry(&output);
    (output, actions.into_inner(), visible)
}

#[test]
fn image_controls_follow_hover_and_keep_button_presses_out_of_native_gestures() {
    for native_edges in [false, true] {
        for (offset, expected) in [
            (0.0, Action::CopyPin(7)),
            (METRICS.button + METRICS.button_gap, Action::ToggleLock(7)),
            (
                2.0 * (METRICS.button + METRICS.button_gap),
                Action::ClosePin(7),
            ),
        ] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            // Cover the emulated top resize edge as well as the image body.
            let row = Rect::from_min_size(bounds.min, METRICS.toolbar_size());
            let point = row.min + egui::vec2(offset + METRICS.button / 2.0, 1.0);
            let (output, actions, visible) =
                paint_composite(&ctx, &texture, bounds, row, vec![], native_edges);
            assert!(!visible);
            assert!(actions.is_empty());
            assert_eq!(image_mesh(&output, &texture), bounds);
            let mut actions = Vec::new();
            for events in [
                vec![Event::PointerMoved(point)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
            ] {
                let (output, emitted, visible) =
                    paint_composite(&ctx, &texture, bounds, row, events, native_edges);
                assert!(visible);
                assert!(commands(&output).is_empty());
                assert_eq!(image_mesh(&output, &texture), bounds);
                actions.extend(emitted);
            }
            assert_eq!(actions.as_slice(), std::slice::from_ref(&expected));
            let (_, actions, visible) = paint_composite(
                &ctx,
                &texture,
                bounds,
                row,
                vec![Event::PointerGone],
                native_edges,
            );
            assert!(!visible);
            assert!(actions.is_empty());
        }
    }
}

#[test]
fn batched_motion_from_controls_does_not_start_drag_or_resize() {
    for native_edges in [false, true] {
        for destination in [egui::pos2(120.0, 80.0), egui::pos2(300.0, 200.0)] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            let row = Rect::from_min_size(bounds.min, METRICS.toolbar_size());
            let point = row.min + egui::vec2(METRICS.button / 2.0, 1.0);
            paint_composite(
                &ctx,
                &texture,
                bounds,
                row,
                vec![Event::PointerMoved(point)],
                native_edges,
            );
            let (output, actions, _) = paint_composite(
                &ctx,
                &texture,
                bounds,
                row,
                vec![pointer(point, true), Event::PointerMoved(destination)],
                native_edges,
            );
            assert!(commands(&output).iter().all(|command| !matches!(
                command,
                ViewportCommand::StartDrag | ViewportCommand::BeginResize(_)
            )));
            assert!(actions.is_empty());
        }
    }
}

#[test]
fn double_clicking_copy_does_not_close_and_escape_emits_one_close() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
    let row = Rect::from_min_size(
        bounds.min + Vec2::splat(METRICS.inset),
        METRICS.toolbar_size(),
    );
    let point = row.min + Vec2::splat(METRICS.button / 2.0);
    paint_composite(
        &ctx,
        &texture,
        bounds,
        row,
        vec![Event::PointerMoved(point)],
        true,
    );
    let mut actions = Vec::new();
    for pressed in [true, false, true, false] {
        let (output, emitted, _) = paint_composite(
            &ctx,
            &texture,
            bounds,
            row,
            vec![pointer(point, pressed)],
            true,
        );
        assert!(commands(&output).is_empty());
        actions.extend(emitted);
    }
    assert_eq!(actions, [Action::CopyPin(7), Action::CopyPin(7)]);
    let (_, actions, _) = paint_composite(
        &ctx,
        &texture,
        bounds,
        row,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        true,
    );
    assert_eq!(actions, [Action::ClosePin(7)]);
}

#[test]
fn native_edges_leave_the_system_cursor_unchanged() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
    for point in [bounds.min + Vec2::splat(1.0), bounds.center()] {
        let (output, _) = paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::PointerMoved(point)],
            true,
            0.0,
            false,
        );
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Default
        );
        assert!(commands(&output).iter().all(|command| !matches!(
            command,
            ViewportCommand::StartDrag | ViewportCommand::BeginResize(_)
        )));
    }
}

#[test]
fn body_press_batched_with_motion_uses_its_actual_origin() {
    for native_edges in [false, true] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
        let point = bounds.center();
        paint_image(
            &ctx,
            &texture,
            bounds,
            vec![Event::PointerMoved(point)],
            native_edges,
            0.0,
            false,
        );
        let (output, _) = paint_image(
            &ctx,
            &texture,
            bounds,
            vec![
                pointer(point, true),
                Event::PointerMoved(bounds.max + Vec2::splat(40.0)),
            ],
            native_edges,
            0.1,
            false,
        );
        assert_eq!(commands(&output), &[ViewportCommand::StartDrag]);
    }
}

#[test]
fn narrow_controls_preserve_the_rightmost_horizontal_actions_without_native_gestures() {
    for (width, offset, expected) in [
        (26.0, 0.0, Action::ClosePin(7)),
        (56.0, 0.0, Action::ToggleLock(7)),
        (
            56.0,
            METRICS.button + METRICS.button_gap,
            Action::ClosePin(7),
        ),
    ] {
        for native_edges in [false, true] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 72.0));
            let row = Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 26.0));
            let point = row.min + egui::vec2(offset + METRICS.button / 2.0, METRICS.button / 2.0);
            let mut actions = Vec::new();
            for events in [
                vec![Event::PointerMoved(point)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
            ] {
                let (output, emitted, visible) =
                    paint_composite(&ctx, &texture, bounds, row, events, native_edges);
                assert!(visible);
                assert_eq!(image_mesh(&output, &texture), bounds);
                assert!(commands(&output).iter().all(|command| !matches!(
                    command,
                    ViewportCommand::StartDrag | ViewportCommand::BeginResize(_)
                )));
                actions.extend(emitted);
            }
            assert_eq!(actions.as_slice(), std::slice::from_ref(&expected));
        }
    }
}

fn secondary(point: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: point,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn menu_item(output: &FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(Rect::from_min_size(text.pos, text.galley.size()).center())
            }
            _ => None,
        })
        .expect("menu item")
}

#[test]
fn independent_menu_dispatches_copy_lock_and_close_inside_its_own_bounds() {
    for (locale, label, expected) in [
        (crate::i18n::Locale::Chinese, "复制", Action::CopyPin(7)),
        (crate::i18n::Locale::Chinese, "锁定", Action::ToggleLock(7)),
        (crate::i18n::Locale::Chinese, "关闭", Action::ClosePin(7)),
        (crate::i18n::Locale::English, "Copy", Action::CopyPin(7)),
        (crate::i18n::Locale::English, "Lock", Action::ToggleLock(7)),
        (crate::i18n::Locale::English, "Close", Action::ClosePin(7)),
        (
            crate::i18n::Locale::Chinese,
            crate::i18n::Locale::Chinese.text(crate::i18n::Text::RestoreSize),
            Action::Zoom(7, vec![Zoom::Original]),
        ),
        (
            crate::i18n::Locale::English,
            crate::i18n::Locale::English.text(crate::i18n::Text::RestoreSize),
            Action::Zoom(7, vec![Zoom::Original]),
        ),
    ] {
        let ctx = egui::Context::default();
        crate::i18n::set_locale(&ctx, locale);
        let bounds = Rect::from_min_size(Pos2::ZERO, MENU_SIZE);
        let actions = RefCell::new(Vec::new());
        let paint = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    events,
                    ..Default::default()
                },
                |ui| menu(ui, 7, false, &|action| actions.borrow_mut().push(action)),
            );
            output.textures_delta.clear();
            output
        };
        let output = paint(vec![]);
        for shape in &output.shapes {
            if let egui::Shape::Text(_) = &shape.shape {
                assert!(bounds.contains_rect(shape.shape.visual_bounding_rect()));
                assert!(
                    shape
                        .clip_rect
                        .contains_rect(shape.shape.visual_bounding_rect())
                );
            }
        }
        let point = menu_item(&output, label);
        assert!(bounds.contains(point));
        for events in [
            vec![Event::PointerMoved(point)],
            vec![pointer(point, true)],
            vec![pointer(point, false)],
        ] {
            let output = paint(events);
            assert!(commands(&output).iter().all(|command| !matches!(
                command,
                ViewportCommand::StartDrag
                    | ViewportCommand::BeginResize(_)
                    | ViewportCommand::InnerSize(_)
                    | ViewportCommand::OuterPosition(_)
            )));
        }
        assert_eq!(actions.into_inner(), [expected]);
    }
}

#[test]
fn restore_original_size_menu_is_disabled_while_locked() {
    for locale in [crate::i18n::Locale::Chinese, crate::i18n::Locale::English] {
        let ctx = egui::Context::default();
        crate::i18n::set_locale(&ctx, locale);
        let actions = RefCell::new(Vec::new());
        let paint = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, MENU_SIZE)),
                    events,
                    ..Default::default()
                },
                |ui| menu(ui, 7, true, &|action| actions.borrow_mut().push(action)),
            );
            output.textures_delta.clear();
            output
        };
        let point = menu_item(&paint(vec![]), locale.text(crate::i18n::Text::RestoreSize));
        for events in [
            vec![Event::PointerMoved(point)],
            vec![pointer(point, true)],
            vec![pointer(point, false)],
        ] {
            assert_no_geometry(&paint(events));
        }
        assert!(actions.into_inner().is_empty());
    }
}

#[test]
fn clicking_the_image_or_unlock_control_keeps_keyboard_commands_available() {
    for native_edges in [false, true] {
        for unlock_control in [false, true] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            let toolbar = snip2pin::pin::toolbar_geometry(bounds, &[], bounds, 1.0).unwrap();
            let point = if unlock_control {
                toolbar.bounds.min
                    + egui::vec2(
                        METRICS.button + METRICS.button_gap + METRICS.button / 2.0,
                        METRICS.button / 2.0,
                    )
            } else {
                bounds.center()
            };
            let paint = |events, locked, time| {
                let actions = RefCell::new(Vec::new());
                let mut input = egui::RawInput {
                    screen_rect: Some(bounds),
                    events,
                    time: Some(time),
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .focused = Some(true);
                let mut output = ctx.run_ui(input, |ui| {
                    image_with_host(
                        ui,
                        7,
                        &texture,
                        Interaction {
                            native_edges,
                            locked,
                            toolbar: Some(toolbar),
                            keyboard_focus: None,
                            ..Default::default()
                        },
                        &|action| actions.borrow_mut().push(action),
                        |command| ctx.send_viewport_cmd(command),
                    );
                    super::controls(ui, 7, locked, toolbar, &|action| {
                        actions.borrow_mut().push(action)
                    });
                });
                output.textures_delta.clear();
                assert_no_geometry(&output);
                assert_eq!(image_mesh(&output, &texture), bounds);
                actions.into_inner()
            };
            assert!(paint(vec![Event::PointerMoved(point)], unlock_control, 0.0).is_empty());
            assert!(paint(vec![pointer(point, true)], unlock_control, 0.1).is_empty());
            assert_eq!(
                paint(vec![pointer(point, false)], unlock_control, 0.2),
                if unlock_control {
                    vec![Action::ToggleLock(7)]
                } else {
                    vec![]
                }
            );
            assert!(!ctx.text_edit_focused());
            for keyboard_focus in [false, true] {
                if keyboard_focus {
                    // Pointer clicks need not take egui focus. Tab does, and
                    // focusing an image or button must keep pin keys usable.
                    assert!(
                        paint(
                            [true, false]
                                .map(|pressed| Event::Key {
                                    key: egui::Key::Tab,
                                    physical_key: None,
                                    pressed,
                                    repeat: false,
                                    modifiers: Modifiers::NONE,
                                })
                                .into(),
                            false,
                            5.0,
                        )
                        .is_empty()
                    );
                    assert!(ctx.egui_wants_keyboard_input());
                    assert!(!ctx.text_edit_focused());
                }
                for (index, (key, expected)) in [
                    (egui::Key::Plus, Action::Zoom(7, vec![Zoom::In])),
                    (egui::Key::ArrowRight, Action::Move(7, [1, 0])),
                    (egui::Key::Num0, Action::Zoom(7, vec![Zoom::Original])),
                ]
                .into_iter()
                .enumerate()
                {
                    let events = [true, false].map(|pressed| Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    });
                    let time = if keyboard_focus { 6.0 } else { 1.0 } + index as f64;
                    assert_eq!(paint(events.into(), false, time), [expected]);
                }
            }
        }
    }
}

#[test]
fn wheel_units_and_precision_are_independent_of_ui_zoom() {
    for zoom in [1.0, 1.25, 2.0] {
        for fast in [false, true] {
            for unit in [
                egui::MouseWheelUnit::Line,
                egui::MouseWheelUnit::Point,
                egui::MouseWheelUnit::Page,
            ] {
                let ctx = egui::Context::default();
                ctx.set_zoom_factor(zoom);
                let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
                warmup.textures_delta.clear();
                let line = ctx.options(|options| options.input_options.line_scroll_speed);
                let delta = match unit {
                    egui::MouseWheelUnit::Line => 1.0,
                    egui::MouseWheelUnit::Point => line / zoom,
                    egui::MouseWheelUnit::Page => line / 160.0,
                };
                let texture = texture(&ctx);
                let actions = RefCell::new(Vec::new());
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(240.0, 160.0) / zoom,
                        )),
                        events: vec![
                            Event::PointerMoved(egui::pos2(40.0, 40.0) / zoom),
                            Event::MouseWheel {
                                unit,
                                delta: egui::vec2(0.0, delta),
                                phase: egui::TouchPhase::Move,
                                modifiers: if fast {
                                    Modifiers::SHIFT
                                } else {
                                    Modifiers::NONE
                                },
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| {
                        image_with_host(
                            ui,
                            7,
                            &texture,
                            Interaction::default(),
                            &|action| actions.borrow_mut().push(action),
                            |command| ctx.send_viewport_cmd(command),
                        );
                    },
                );
                output.textures_delta.clear();
                assert_no_geometry(&output);
                let actions = actions.into_inner();
                if unit == egui::MouseWheelUnit::Point {
                    assert!(actions.is_empty());
                    continue;
                }
                let [Action::Zoom(7, commands)] = actions.as_slice() else {
                    panic!("expected one wheel adjustment: {actions:?}")
                };
                let [Zoom::At { factor, .. }] = commands.as_slice() else {
                    panic!("expected one scale: {commands:?}")
                };
                let expected = if fast { 1.25 } else { 1.01 };
                assert!(
                    (factor - expected).abs() < 0.00001,
                    "zoom={zoom} unit={unit:?} fast={fast} factor={factor}"
                );
            }
        }
    }
}

#[test]
fn wheel_batches_preserve_reset_order_and_ignore_reserved_or_excluded_input() {
    for blocked in [
        "none",
        "reset",
        "ctrl",
        "command",
        "alt",
        "toolbar",
        "drag",
        "horizontal",
        "cancel",
        "editing",
    ] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let actions = RefCell::new(Vec::new());
        let point = egui::pos2(40.0, 40.0);
        let modifiers = match blocked {
            "ctrl" => Modifiers::CTRL,
            "command" => Modifiers::COMMAND,
            "alt" => Modifiers::ALT,
            _ => Modifiers::NONE,
        };
        let wheel = || Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: match blocked {
                "horizontal" => egui::vec2(1.0, 0.0),
                _ => egui::vec2(0.0, 0.5),
            },
            phase: if blocked == "cancel" {
                egui::TouchPhase::Cancel
            } else {
                egui::TouchPhase::Move
            },
            modifiers,
        };
        let mut events = vec![Event::PointerMoved(point)];
        if blocked == "drag" {
            events.push(pointer(point, true));
        }
        events.extend([wheel(), wheel()]);
        if blocked == "reset" {
            events.push(Event::Key {
                key: egui::Key::Num0,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            events.push(wheel());
        }
        let mut name = "pin".to_owned();
        if blocked == "editing" {
            let mut warmup = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
                    ..Default::default()
                },
                |ui| {
                    ui.add(egui::TextEdit::singleline(&mut name))
                        .request_focus();
                },
            );
            warmup.textures_delta.clear();
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
                events,
                ..Default::default()
            },
            |ui| {
                if blocked == "editing" {
                    ui.add(egui::TextEdit::singleline(&mut name))
                        .request_focus();
                }
                image_with_host(
                    ui,
                    7,
                    &texture,
                    Interaction {
                        keyboard_focus: Some(blocked == "reset"),
                        toolbar: (blocked == "toolbar").then_some(Toolbar {
                            bounds: Rect::from_center_size(point, Vec2::splat(20.0)),
                            count: 0,
                        }),
                        ..Default::default()
                    },
                    &|action| actions.borrow_mut().push(action),
                    |_| {},
                );
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("wheel events batch once");
                }
            },
        );
        output.textures_delta.clear();
        assert_no_geometry(&output);
        let actions = actions.into_inner();
        if blocked == "none" {
            assert!(
                matches!(actions.as_slice(), [Action::Zoom(7, commands)] if matches!(commands.as_slice(), [Zoom::At { factor, .. }] if (*factor - 1.01).abs() < 0.00001))
            );
        } else if blocked == "reset" {
            assert!(
                matches!(actions.as_slice(), [Action::Zoom(7, commands)] if matches!(commands.as_slice(), [Zoom::At { factor: before, .. }, Zoom::Original, Zoom::At { factor: after, .. }] if (*before - 1.01).abs() < 0.00001 && (*after - 1.01_f32.sqrt()).abs() < 0.00001))
            );
        } else if blocked == "ctrl" {
            assert!(
                matches!(actions.as_slice(), [Action::Zoom(7, commands)] if matches!(commands.as_slice(), [Zoom::At { factor, .. }] if *factor > 1.0))
            );
        } else {
            assert!(actions.is_empty(), "blocked={blocked}: {actions:?}");
        }
    }
}

#[test]
fn wheel_zoom_tracks_hover_without_requiring_keyboard_focus() {
    for focused in [false, true] {
        for locked in [false, true] {
            for hovering in [false, true] {
                let ctx = egui::Context::default();
                let texture = texture(&ctx);
                let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
                let actions = RefCell::new(Vec::new());
                let mut input = egui::RawInput {
                    screen_rect: Some(bounds),
                    events: vec![
                        Event::PointerMoved(if hovering {
                            bounds.center()
                        } else {
                            bounds.max + Vec2::splat(10.0)
                        }),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Line,
                            delta: egui::vec2(0.0, 1.0),
                            phase: egui::TouchPhase::Move,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .focused = Some(focused);
                let mut output = ctx.run_ui(input, |ui| {
                    image_with_host(
                        ui,
                        7,
                        &texture,
                        Interaction {
                            locked,
                            ..Default::default()
                        },
                        &|action| actions.borrow_mut().push(action),
                        |command| ctx.send_viewport_cmd(command),
                    );
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("wheel dispatch must happen once");
                    }
                });
                output.textures_delta.clear();
                assert_no_geometry(&output);
                let actions = actions.into_inner();
                assert_eq!(actions.len(), usize::from(hovering && !locked));
                if hovering && !locked {
                    assert!(
                        matches!(&actions[0], Action::Zoom(7, commands) if commands.len() == 1)
                    );
                }
            }
        }
    }
}

#[test]
fn shift_arrow_taps_accumulate_fast_movement_once() {
    let ctx = egui::Context::default();
    let texture = texture(&ctx);
    let actions = RefCell::new(Vec::new());
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
        events: [egui::Key::ArrowRight, egui::Key::ArrowRight]
            .into_iter()
            .flat_map(|key| {
                [true, false].map(|pressed| Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::SHIFT,
                })
            })
            .collect(),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .focused = Some(true);
    let mut output = ctx.run_ui(input, |ui| {
        image_with_host(
            ui,
            7,
            &texture,
            Interaction::default(),
            &|action| actions.borrow_mut().push(action),
            |command| ctx.send_viewport_cmd(command),
        );
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx().request_discard("fast keys dispatch once");
        }
    });
    output.textures_delta.clear();
    assert_no_geometry(&output);
    assert_eq!(actions.into_inner(), vec![Action::Move(7, [100, 0])]);
}

#[test]
fn keyboard_zoom_and_movement_require_native_focus_and_an_unlocked_pin() {
    for focused in [None, Some(false), Some(true)] {
        for locked in [false, true] {
            for native_edges in [false, true] {
                for (key, expected) in [
                    (egui::Key::Plus, Action::Zoom(7, vec![Zoom::In])),
                    (egui::Key::Equals, Action::Zoom(7, vec![Zoom::In])),
                    (egui::Key::Minus, Action::Zoom(7, vec![Zoom::Out])),
                    (egui::Key::Num0, Action::Zoom(7, vec![Zoom::Original])),
                    (egui::Key::ArrowLeft, Action::Move(7, [-1, 0])),
                    (egui::Key::ArrowRight, Action::Move(7, [1, 0])),
                    (egui::Key::ArrowUp, Action::Move(7, [0, -1])),
                    (egui::Key::ArrowDown, Action::Move(7, [0, 1])),
                ] {
                    let ctx = egui::Context::default();
                    let texture = texture(&ctx);
                    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(1.0, 120.0));
                    let actions = RefCell::new(Vec::new());
                    let mut input = egui::RawInput {
                        screen_rect: Some(bounds),
                        events: vec![Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        }],
                        ..Default::default()
                    };
                    input
                        .viewports
                        .get_mut(&egui::ViewportId::ROOT)
                        .unwrap()
                        .focused = focused;
                    let mut output = ctx.run_ui(input, |ui| {
                        image_with_host(
                            ui,
                            7,
                            &texture,
                            Interaction {
                                native_edges,
                                locked,
                                toolbar: None,
                                keyboard_focus: None,
                                ..Default::default()
                            },
                            &|action| actions.borrow_mut().push(action),
                            |command| ctx.send_viewport_cmd(command),
                        );
                        if ui.ctx().current_pass_index() == 0 {
                            ui.ctx()
                                .request_discard("keyboard actions must dispatch once");
                        }
                    });
                    output.textures_delta.clear();
                    assert_no_geometry(&output);
                    assert_eq!(image_mesh(&output, &texture), bounds);
                    assert_eq!(texture.size(), [2, 2]);
                    assert_eq!(
                        actions.into_inner(),
                        if focused == Some(true) && !locked {
                            vec![expected]
                        } else {
                            vec![]
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn keyboard_commands_batch_in_order_and_cancel_movement_once_across_passes() {
    for (keys, expected) in [
        (
            [egui::Key::Plus, egui::Key::Plus],
            vec![Action::Zoom(7, vec![Zoom::In, Zoom::In])],
        ),
        (
            [egui::Key::Num0, egui::Key::Plus],
            vec![Action::Zoom(7, vec![Zoom::Original, Zoom::In])],
        ),
        (
            [egui::Key::ArrowRight, egui::Key::ArrowRight],
            vec![Action::Move(7, [2, 0])],
        ),
        (
            [egui::Key::ArrowLeft, egui::Key::ArrowDown],
            vec![Action::Move(7, [-1, 1])],
        ),
        ([egui::Key::ArrowLeft, egui::Key::ArrowRight], vec![]),
        ([egui::Key::ArrowUp, egui::Key::ArrowDown], vec![]),
        (
            [egui::Key::Num0, egui::Key::ArrowRight],
            vec![
                Action::Zoom(7, vec![Zoom::Original]),
                Action::Move(7, [1, 0]),
            ],
        ),
    ] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(1.0, 120.0));
        let actions = RefCell::new(Vec::new());
        let events = keys
            .into_iter()
            .flat_map(|key| {
                [true, false].map(|pressed| Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                })
            })
            .collect();
        let mut input = egui::RawInput {
            screen_rect: Some(bounds),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .focused = Some(true);
        let mut output = ctx.run_ui(input, |ui| {
            image_with_host(
                ui,
                7,
                &texture,
                Interaction::default(),
                &|action| actions.borrow_mut().push(action),
                |command| ctx.send_viewport_cmd(command),
            );
            if ui.ctx().current_pass_index() == 0 {
                ui.ctx()
                    .request_discard("batched keyboard commands must dispatch once");
            }
        });
        output.textures_delta.clear();
        assert_eq!(output.platform_output.num_completed_passes, 2);
        assert_no_geometry(&output);
        assert_eq!(image_mesh(&output, &texture), bounds);
        assert_eq!(actions.into_inner(), expected);
    }
}

#[test]
fn keyboard_zoom_and_movement_leave_text_editing_and_command_shortcuts_alone() {
    for editing in [false, true] {
        for modifiers in [
            Modifiers::NONE,
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
            Modifiers {
                command: true,
                mac_cmd: true,
                ..Modifiers::NONE
            },
            Modifiers {
                alt: true,
                ..Modifiers::NONE
            },
            Modifiers {
                shift: true,
                ..Modifiers::NONE
            },
        ] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            let actions = RefCell::new(Vec::new());
            let mut name = "Reference".to_owned();
            for pressed in [false, true] {
                let mut input = egui::RawInput {
                    screen_rect: Some(bounds),
                    events: [egui::Key::Plus, egui::Key::ArrowLeft]
                        .into_iter()
                        .map(|key| Event::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: pressed,
                            modifiers,
                        })
                        .collect(),
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .focused = Some(true);
                let mut output = ctx.run_ui(input, |ui| {
                    if editing {
                        ui.add(egui::TextEdit::singleline(&mut name))
                            .request_focus();
                    }
                    image_with_host(
                        ui,
                        7,
                        &texture,
                        Interaction::default(),
                        &|action| actions.borrow_mut().push(action),
                        |command| ctx.send_viewport_cmd(command),
                    );
                });
                output.textures_delta.clear();
                assert_no_geometry(&output);
            }
            assert_eq!(
                actions.into_inner(),
                if !editing && (modifiers == Modifiers::NONE || modifiers.shift) {
                    if modifiers.shift {
                        vec![
                            Action::Zoom(7, vec![Zoom::Scale(snip2pin::pin::FAST_ZOOM_FACTOR)]),
                            Action::Move(7, [-50, 0]),
                        ]
                    } else {
                        vec![Action::Zoom(7, vec![Zoom::In]), Action::Move(7, [-1, 0])]
                    }
                } else {
                    vec![]
                }
            );
        }
    }
}

#[test]
fn focused_resize_zone_expands_inward_but_preserves_center_dragging() {
    for focused in [false, true] {
        for native_edges in [false, true] {
            for size in [egui::vec2(240.0, 160.0), egui::vec2(3.0, 3.0)] {
                let bounds = Rect::from_min_size(Pos2::ZERO, size);
                let edge = egui::pos2(
                    ((METRICS.inset + PIN.focused_resize_inset) / 2.0).min(size.x / 6.0),
                    size.y / 2.0,
                );
                for point in [edge, bounds.center()] {
                    let ctx = egui::Context::default();
                    let texture = texture(&ctx);
                    let mut last_output = None;
                    for events in [vec![Event::PointerMoved(point)], vec![pointer(point, true)]] {
                        let mut input = egui::RawInput {
                            screen_rect: Some(bounds),
                            events,
                            ..Default::default()
                        };
                        input
                            .viewports
                            .get_mut(&egui::ViewportId::ROOT)
                            .unwrap()
                            .focused = Some(focused);
                        let mut output = ctx.run_ui(input, |ui| {
                            image(ui, 7, &texture, native_edges, None, &|_| {});
                        });
                        output.textures_delta.clear();
                        assert_no_geometry(&output);
                        assert_eq!(image_mesh(&output, &texture), bounds);
                        last_output = Some(output);
                    }
                    let expected = if !native_edges
                        && point == edge
                        && (focused || size.x < METRICS.inset * 3.0)
                    {
                        ViewportCommand::BeginResize(ResizeDirection::West)
                    } else {
                        ViewportCommand::StartDrag
                    };
                    assert_eq!(commands(&last_output.unwrap()), &[expected]);
                }
            }
        }
    }
}

#[test]
fn right_click_dispatches_one_menu_request_on_image_and_hover_buttons() {
    for native_edges in [false, true] {
        for (size, controls_rect, point) in [
            (egui::vec2(3.0, 3.0), None, egui::pos2(1.0, 1.0)),
            (
                egui::vec2(240.0, 160.0),
                Some(Rect::from_min_size(Pos2::ZERO, METRICS.toolbar_size())),
                egui::pos2(13.0, 13.0),
            ),
            (
                egui::vec2(26.0, 72.0),
                Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(26.0, 26.0))),
                egui::pos2(13.0, 13.0),
            ),
        ] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, size);
            let actions = RefCell::new(Vec::new());
            for (index, events) in [
                vec![Event::PointerMoved(point)],
                vec![secondary(point, true)],
                vec![secondary(point, false)],
                vec![],
            ]
            .into_iter()
            .enumerate()
            {
                let mut painted = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        events,
                        time: Some(index as f64 * 0.1),
                        ..Default::default()
                    },
                    |ui| {
                        image(ui, 7, &texture, native_edges, controls_rect, &|action| {
                            actions.borrow_mut().push(action)
                        });
                        if let Some(row) = controls_rect {
                            ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
                                controls(ui, 7, &|action| actions.borrow_mut().push(action))
                            });
                        }
                        if index == 2 && ui.ctx().current_pass_index() == 0 {
                            ui.ctx()
                                .request_discard("menu request is dispatched only once");
                        }
                    },
                );
                painted.textures_delta.clear();
                assert_eq!(image_mesh(&painted, &texture), bounds);
                assert!(commands(&painted).iter().all(|command| !matches!(
                    command,
                    ViewportCommand::StartDrag | ViewportCommand::BeginResize(_)
                )));
            }
            assert_eq!(actions.into_inner(), [Action::MenuPin(7)]);
        }
    }
}

#[test]
fn narrow_text_image_keeps_its_displayed_dimensions_with_only_close_visible() {
    let ctx = egui::Context::default();
    let mut text_image = egui::ColorImage::filled([32, 72], Color32::WHITE);
    // Generated 3×5 glyphs spell TEXT, so this fixture catches enlargement of
    // narrow text captures rather than approving a larger window for buttons.
    for (index, rows) in [
        [0b111, 0b010, 0b010, 0b010, 0b010],
        [0b111, 0b100, 0b110, 0b100, 0b111],
        [0b101, 0b101, 0b010, 0b101, 0b101],
        [0b111, 0b010, 0b010, 0b010, 0b010],
    ]
    .into_iter()
    .enumerate()
    {
        for (y, row) in rows.into_iter().enumerate() {
            for x in 0..3 {
                if row & (1 << (2 - x)) != 0 {
                    text_image[(2 + index * 4 + x, 2 + y)] = Color32::BLACK;
                }
            }
        }
    }
    let texture = ctx.load_texture(
        "narrow-generated-text",
        text_image,
        egui::TextureOptions::NEAREST,
    );
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(32.0, 72.0));
    let toolbar = snip2pin::pin::toolbar_geometry(bounds, &[], bounds, 1.0).expect("close fits");
    assert_eq!(toolbar.count, 1);
    assert_eq!(toolbar.bounds.size(), egui::vec2(26.0, 26.0));
    let (output, actions, visible) = paint_composite(
        &ctx,
        &texture,
        bounds,
        toolbar.bounds,
        vec![Event::PointerMoved(bounds.center())],
        true,
    );
    assert!(visible);
    assert!(actions.is_empty());
    assert_eq!(image_mesh(&output, &texture).size(), egui::vec2(32.0, 72.0));
    assert_eq!(texture.size(), [32, 72]);
}

#[test]
fn tiny_image_right_click_does_not_enlarge_the_image_or_start_a_native_gesture() {
    for size in [egui::vec2(3.0, 3.0), egui::vec2(14.0, 120.0)] {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, size);
        for (index, events) in [
            vec![Event::PointerMoved(bounds.center())],
            vec![secondary(bounds.center(), true)],
            vec![secondary(bounds.center(), false)],
        ]
        .into_iter()
        .enumerate()
        {
            let (output, actions) = paint_image(
                &ctx,
                &texture,
                bounds,
                events,
                true,
                index as f64 * 0.1,
                false,
            );
            assert_eq!(
                actions,
                if index == 2 {
                    vec![Action::MenuPin(7)]
                } else {
                    vec![]
                }
            );
            assert_eq!(image_mesh(&output, &texture), bounds);
            assert!(commands(&output).iter().all(|command| !matches!(
                command,
                ViewportCommand::StartDrag | ViewportCommand::BeginResize(_)
            )));
        }
    }
}

#[test]
fn locked_images_ignore_drag_resize_double_click_and_escape_but_keep_the_menu() {
    for native_edges in [false, true] {
        for point in [egui::pos2(1.0, 1.0), egui::pos2(120.0, 80.0)] {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            for (index, events) in [
                vec![Event::PointerMoved(point)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
                vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                vec![secondary(point, true)],
                vec![secondary(point, false)],
            ]
            .into_iter()
            .enumerate()
            {
                let actions = RefCell::new(Vec::new());
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        events,
                        time: Some(index as f64 * 0.05),
                        ..Default::default()
                    },
                    |ui| {
                        let ctx = ui.ctx().clone();
                        image_with_host(
                            ui,
                            7,
                            &texture,
                            Interaction {
                                native_edges,
                                locked: true,
                                toolbar: None,
                                keyboard_focus: None,
                                ..Default::default()
                            },
                            &|action| actions.borrow_mut().push(action),
                            |command| ctx.send_viewport_cmd(command),
                        );
                    },
                );
                output.textures_delta.clear();
                assert_eq!(image_mesh(&output, &texture), bounds);
                // egui synchronizes the native theme on a context's first frame.
                let expected_commands = if index == 0 {
                    vec![ViewportCommand::SetTheme(egui::SystemTheme::SystemDefault)]
                } else {
                    vec![]
                };
                assert_eq!(commands(&output), expected_commands.as_slice());
                assert_eq!(
                    output.platform_output.cursor_icon,
                    egui::CursorIcon::Default
                );
                assert_eq!(
                    actions.into_inner(),
                    if index == 7 {
                        vec![Action::MenuPin(7)]
                    } else if index == 1 || index == 3 {
                        vec![Action::UnlockHint(7)]
                    } else {
                        vec![]
                    }
                );
            }
            let actions = RefCell::new(Vec::new());
            let mut input = egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events = vec![egui::ViewportEvent::Close];
            let mut output = ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                image_with_host(
                    ui,
                    7,
                    &texture,
                    Interaction {
                        native_edges,
                        locked: true,
                        toolbar: None,
                        keyboard_focus: None,
                        ..Default::default()
                    },
                    &|action| actions.borrow_mut().push(action),
                    |command| ctx.send_viewport_cmd(command),
                );
            });
            output.textures_delta.clear();
            assert_eq!(commands(&output), &[ViewportCommand::CancelClose]);
            assert!(actions.into_inner().is_empty());
        }
    }
}

#[test]
fn locked_controls_allow_copy_and_unlock_while_close_stays_disabled() {
    for (size, cases) in [
        (
            egui::vec2(86.0, 26.0),
            vec![Some(Action::CopyPin(7)), Some(Action::ToggleLock(7)), None],
        ),
        (
            egui::vec2(56.0, 26.0),
            vec![Some(Action::ToggleLock(7)), None],
        ),
        (egui::vec2(26.0, 26.0), vec![None]),
    ] {
        for (index, expected) in cases.into_iter().enumerate() {
            let ctx = egui::Context::default();
            let texture = texture(&ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
            let row = Rect::from_min_size(bounds.min, size);
            let toolbar = snip2pin::pin::toolbar_geometry(row, &[], row, 1.0).unwrap();
            let point = row.min
                + egui::vec2(
                    index as f32 * (METRICS.button + METRICS.button_gap) + METRICS.button / 2.0,
                    1.0,
                );
            let actions = RefCell::new(Vec::new());
            for (frame, events) in [
                vec![Event::PointerMoved(point)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
            ]
            .into_iter()
            .enumerate()
            {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let ctx = ui.ctx().clone();
                        image_with_host(
                            ui,
                            7,
                            &texture,
                            Interaction {
                                native_edges: false,
                                locked: true,
                                toolbar: Some(toolbar),
                                keyboard_focus: None,
                                ..Default::default()
                            },
                            &|action| actions.borrow_mut().push(action),
                            |command| ctx.send_viewport_cmd(command),
                        );
                        super::controls(ui, 7, true, toolbar, &|action| {
                            actions.borrow_mut().push(action)
                        });
                    },
                );
                output.textures_delta.clear();
                let expected_commands = if frame == 0 {
                    vec![ViewportCommand::SetTheme(egui::SystemTheme::SystemDefault)]
                } else {
                    vec![]
                };
                assert_eq!(commands(&output), expected_commands.as_slice());
                assert_eq!(image_mesh(&output, &texture), bounds);
            }
            assert_eq!(
                actions.into_inner(),
                expected.into_iter().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn locked_menu_keeps_copy_and_unlock_available_and_close_disabled_in_both_locales() {
    for (locale, copy, unlock, close) in [
        (crate::i18n::Locale::Chinese, "复制", "解锁", "关闭"),
        (crate::i18n::Locale::English, "Copy", "Unlock", "Close"),
    ] {
        for (label, expected) in [
            (copy, Some(Action::CopyPin(7))),
            (unlock, Some(Action::ToggleLock(7))),
            (close, None),
        ] {
            let ctx = egui::Context::default();
            crate::i18n::set_locale(&ctx, locale);
            let bounds = Rect::from_min_size(Pos2::ZERO, MENU_SIZE);
            let actions = RefCell::new(Vec::new());
            let paint = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        events,
                        ..Default::default()
                    },
                    |ui| menu(ui, 7, true, &|action| actions.borrow_mut().push(action)),
                );
                output.textures_delta.clear();
                output
            };
            let output = paint(vec![]);
            let copy_point = menu_item(&output, copy);
            let unlock_point = menu_item(&output, unlock);
            let close_point = menu_item(&output, close);
            assert!(copy_point.y < unlock_point.y && unlock_point.y < close_point.y);
            for point in [copy_point, unlock_point, close_point] {
                assert!(bounds.contains(point));
            }
            let point = menu_item(&output, label);
            for events in [
                vec![Event::PointerMoved(point)],
                vec![pointer(point, true)],
                vec![pointer(point, false)],
            ] {
                assert!(commands(&paint(events)).is_empty());
            }
            assert_eq!(
                actions.into_inner(),
                expected.into_iter().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn hiding_a_button_during_a_press_cannot_trigger_its_replacement() {
    let ctx = egui::Context::default();
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0));
    let actions = RefCell::new(Vec::new());
    let paint = |count, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                events,
                ..Default::default()
            },
            |ui| {
                super::controls(
                    ui,
                    7,
                    false,
                    Toolbar {
                        bounds: Rect::from_min_size(Pos2::ZERO, METRICS.toolbar_size_for(count)),
                        count,
                    },
                    &|action| actions.borrow_mut().push(action),
                )
            },
        );
        output.textures_delta.clear();
    };
    let point = egui::pos2(METRICS.button / 2.0, METRICS.button / 2.0);
    paint(3, vec![Event::PointerMoved(point)]);
    paint(3, vec![pointer(point, true)]);
    paint(2, vec![pointer(point, false)]);
    assert!(
        actions.into_inner().is_empty(),
        "releasing Copy after it disappears must not toggle Lock"
    );
}
