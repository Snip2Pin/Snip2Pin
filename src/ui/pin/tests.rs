use super::*;
use Action as Event;
use egui::Vec2;
use std::sync::mpsc;

fn resize_trace(start: fn(&Rect) -> Pos2, delta: Vec2) -> (Rect, Rect, Vec<ViewportCommand>) {
    resize_trace_with_host(start, delta, false, false)
}

fn resize_trace_with_host(
    start: fn(&Rect) -> Pos2,
    delta: Vec2,
    synchronous: bool,
    shift: bool,
) -> (Rect, Rect, Vec<ViewportCommand>) {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "trace",
        egui::ColorImage::filled([2, 2], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    let (events, _) = mpsc::channel();
    let area = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0));
    let initial = snip2pin::pin::host_frame(frame_geometry(
        Placement {
            position: [160.0, 160.0],
            size: [240.0, 160.0],
        },
        area,
        1.0,
        1.0,
    ));
    let host = initial.window;
    let mut prepared = Preparation::default();
    let _ = prepared.update(initial, Some(host), Some(host), 1.0);
    let press = start(&initial.image);
    let target = press + delta;
    let mut last = None;
    let modifiers = egui::Modifiers {
        shift,
        ..egui::Modifiers::NONE
    };
    for (point, mut events_in) in [
        (press, vec![egui::Event::PointerMoved(press)]),
        (
            press,
            vec![egui::Event::PointerButton {
                pos: press,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
        ),
        (target, vec![egui::Event::PointerMoved(target)]),
    ] {
        events_in.insert(0, egui::Event::ModifiersChanged(modifiers));
        let Prepared::Paint { frame, .. } = prepared.update(initial, Some(host), Some(host), 1.0)
        else {
            panic!("already initialized")
        };
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, host.size())),
            events: events_in,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root")
            .inner_rect = Some(host);
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root")
            .outer_rect = Some(host);
        let mut painted_host = host;
        let mut output = ctx.run_ui(input, |ui| {
            show_with_host(
                ui,
                Model {
                    id: 1,
                    texture: &texture,
                    frame,
                    events: &|event| {
                        let _ = events.send(event);
                    },
                    monitors: &[area],
                    coordinate_ratio: 1.0,
                    minimum_size: Vec2::splat(1.0),
                    surface: Surface::Native,
                },
                &mut prepared,
                || Some((point + host.min.to_vec2()).into()),
                |frame| {
                    if synchronous {
                        painted_host = frame.window;
                        Some(frame.window)
                    } else {
                        None
                    }
                },
            );
        });
        output.textures_delta.clear();
        let painted = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                    if synchronous {
                        assert!(
                            shape.clip_rect.contains_rect(mesh.calc_bounds()),
                            "new image must not retain the old host clip"
                        );
                    }
                    Some(mesh.calc_bounds())
                }
                _ => None,
            })
            .expect("painted image");
        let commands = output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .clone();
        // eframe swaps this image before dispatching viewport commands.
        last = Some((painted.translate(painted_host.min.to_vec2()), commands));
    }
    let (painted, commands) = last.expect("trace");
    (
        initial.image.translate(host.min.to_vec2()),
        painted,
        commands,
    )
}

#[test]
fn resized_content_and_host_change_in_the_same_frame_without_pivot_jitter() {
    let (original, painted, _) = resize_trace(Rect::left_top, egui::vec2(-30.0, -20.0));
    assert_eq!(
        painted.max, original.max,
        "the fixed opposite corner cannot jump when the host moves"
    );
    assert_eq!(
        painted.size(),
        original.size() + egui::vec2(30.0, 20.0),
        "paint cannot lag one resize event behind the host"
    );
}

#[test]
fn synchronous_layout_precedes_paint_and_updates_the_old_clip() {
    let (original, painted, commands) =
        resize_trace_with_host(Rect::left_top, egui::vec2(-240.0, -160.0), true, false);
    assert_eq!(painted.max, original.max);
    assert_eq!(painted.size(), original.size() + egui::vec2(240.0, 160.0));
    assert!(commands.iter().all(|command| !matches!(
        command,
        ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
    )));
}

#[test]
fn every_image_edge_can_resize_without_moving_the_whole_pin() {
    for (point, delta) in [
        (
            Rect::left_center as fn(&Rect) -> Pos2,
            egui::vec2(-30.0, 0.0),
        ),
        (Rect::right_center, egui::vec2(30.0, 0.0)),
        (Rect::center_top, egui::vec2(0.0, -30.0)),
        (Rect::center_bottom, egui::vec2(0.0, 30.0)),
    ] {
        let (original, proportional, commands) = resize_trace(point, delta);
        assert!((original.aspect_ratio() - proportional.aspect_ratio()).abs() < 0.001);
        let (_, free, _) = resize_trace_with_host(point, delta, false, true);
        assert_eq!(free.size(), original.size() + delta.abs());
        if point(&original) == original.right_center() {
            assert_eq!(proportional.left_top(), original.left_top());
            assert_eq!(free.left_top(), original.left_top());
        }
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, ViewportCommand::InnerSize(_))),
            "dragging an edge must resize instead of translating the image"
        );
    }
}

#[test]
fn bottom_left_and_top_right_drags_keep_the_opposite_painted_corner_fixed() {
    for shift in [false, true] {
        let (original, painted, _) =
            resize_trace_with_host(Rect::left_bottom, egui::vec2(-45.0, 20.0), true, shift);
        assert_eq!(painted.right_top(), original.right_top());
        let (original, painted, _) =
            resize_trace_with_host(Rect::right_top, egui::vec2(45.0, -20.0), true, shift);
        assert_eq!(painted.left_bottom(), original.left_bottom());
    }
}

#[test]
fn corner_drag_defaults_to_proportional_resize_and_shift_allows_free_resize() {
    for (proportional, global_cursor, known_position) in [
        (false, true, true),
        (true, true, true),
        (false, false, true),
        (true, false, true),
        (false, false, false),
        (true, false, false),
    ] {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "resize",
            egui::ColorImage::filled([2, 2], Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let (sender, _) = mpsc::channel();
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(680.0, 420.0));
        let frame = frame_geometry(
            Placement {
                position: [100.0, 100.0],
                size: [200.0, 100.0],
            },
            screen,
            1.0,
            1.0,
        );
        let mut prepared = Preparation::default();
        let _ = prepared.update(frame, Some(frame.window), Some(frame.window), 1.0);
        let _ = prepared.update(frame, Some(frame.window), Some(frame.window), 1.0);
        let press = frame.image.right_bottom();
        let target = press + egui::vec2(40.0, 10.0);
        let modifiers = egui::Modifiers {
            shift: !proportional,
            ..egui::Modifiers::NONE
        };
        let mut resized = false;
        for (point, mut events) in [
            (press, vec![egui::Event::PointerMoved(press)]),
            (
                press,
                vec![egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                }],
            ),
            (target, vec![egui::Event::PointerMoved(target)]),
            (
                target,
                vec![egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                }],
            ),
        ] {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, frame.window.size())),
                events,
                ..Default::default()
            };
            input.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    inner_rect: known_position.then_some(frame.window),
                    outer_rect: known_position.then_some(frame.window),
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |ui| {
                show(
                    ui,
                    Model {
                        id: 1,
                        texture: &texture,
                        frame,
                        events: &|event| {
                            let _ = sender.send(event);
                        },
                        monitors: &[screen],
                        coordinate_ratio: 1.0,
                        minimum_size: Vec2::splat(1.0),
                        surface: Surface::Native,
                    },
                    &mut prepared,
                    || global_cursor.then_some((point + frame.window.min.to_vec2()).into()),
                )
            });
            resized |= output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, ViewportCommand::InnerSize(_)));
            output.textures_delta.clear();
        }
        assert!(resized, "corner interaction must dispatch a resize");
        assert!(!prepared.resize.is_active());
        let Prepared::Paint { frame: painted, .. } =
            prepared.update(frame, Some(frame.window), Some(frame.window), 1.0)
        else {
            panic!("paint after resize");
        };
        assert_eq!(
            painted.image.size(),
            if proportional {
                egui::vec2(240.0, 120.0)
            } else {
                egui::vec2(240.0, 110.0)
            }
        );
        assert_eq!(
            painted.image.min, frame.image.min,
            "bottom-right resizing leaves the opposite corner fixed"
        );
    }
}

#[test]
fn upward_drag_respects_system_bounds_and_keeps_lower_buttons_reachable() {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "pin",
        egui::ColorImage::filled([2, 2], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    let (sender, _) = mpsc::channel();
    let screen = Rect::from_min_size(egui::pos2(0.0, 28.0), egui::vec2(680.0, 420.0));
    let frame = frame_geometry(
        Placement {
            position: [100.0, 100.0],
            size: [193.0, 121.0],
        },
        screen,
        1.0,
        1.0,
    );
    let press = frame.image.center();
    let cursor = egui::pos2(frame.window.min.x + press.x, 0.0);
    let local = cursor - frame.window.min.to_vec2();
    let mut drag = Preparation::default();
    let mut requested = None;
    for events in [
        vec![egui::Event::PointerMoved(press)],
        vec![egui::Event::PointerButton {
            pos: press,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerMoved(local)],
    ] {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, frame.window.size())),
            events,
            ..Default::default()
        };
        input.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                inner_rect: Some(frame.window),
                outer_rect: Some(frame.window),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| {
            show(
                ui,
                Model {
                    id: 1,
                    texture: &texture,
                    frame,
                    events: &|event| {
                        let _ = sender.send(event);
                    },
                    monitors: &[],
                    coordinate_ratio: 1.0,
                    minimum_size: Vec2::splat(1.0),
                    surface: Surface::Native,
                },
                &mut drag,
                || Some(cursor.into()),
            )
        });
        output.textures_delta.clear();
        for command in &output.viewport_output[&egui::ViewportId::ROOT].commands {
            if let ViewportCommand::OuterPosition(position) = command {
                requested = Some(*position);
            }
        }
    }
    let position = requested.expect("drag requests a position");
    let image_origin = drag.image(frame.grid).expect("logical image position").min;
    let image = Rect::from_min_size(image_origin, frame.image.size());
    if cfg!(target_os = "macos") {
        assert_eq!(
            image.min.y, screen.min.y,
            "the picture touches the menu-bar boundary"
        );
    } else {
        assert!(
            image.min.y < 0.0,
            "other backends can move past the physical top"
        );
    }
    assert_eq!(image.size(), frame.image.size());
    let moved = snip2pin::pin::host_frame(frame.moved_to(image_origin, &[screen], 1.0));
    assert_eq!(moved.window.min, position);
    assert!((moved.toolbar_side == ToolbarSide::Bottom));
    let (copy, close) = moved.toolbar_rects();
    for button in [copy, close] {
        assert!(
            screen.contains_rect(button.translate(moved.window.min.to_vec2())),
            "lower controls must remain visible even when the image leaves the screen"
        );
    }
}

#[test]
fn pointer_drag_moves_the_image_to_the_top_without_native_drag_or_idle_retries() {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "pin",
        egui::ColorImage::filled([2, 2], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    let (sender, events) = mpsc::channel();
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(680.0, 420.0));
    let frame = frame_geometry(
        Placement {
            position: [100.0, 100.0],
            size: [193.0, 121.0],
        },
        screen,
        1.0,
        1.0,
    );
    let press = frame.image.center();
    let target = egui::pos2(frame.window.min.x, -frame.image.min.y);
    let global = target + press.to_vec2();
    let local = global - frame.window.min.to_vec2();
    let mut drag = Preparation::default();
    for (index, input_events) in [
        vec![egui::Event::PointerMoved(press)],
        vec![egui::Event::PointerButton {
            pos: press,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerMoved(local)],
        vec![egui::Event::PointerMoved(local)],
        vec![],
        vec![egui::Event::PointerButton {
            pos: local,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, frame.window.size())),
            events: input_events,
            ..Default::default()
        };
        input.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                inner_rect: Some(frame.window),
                outer_rect: Some(frame.window),
                ..Default::default()
            },
        );
        let mut cursor_reads = 0;
        let mut output = ctx.run_ui(input, |ui| {
            show(
                ui,
                Model {
                    id: 1,
                    texture: &texture,
                    frame,
                    events: &|event| {
                        let _ = sender.send(event);
                    },
                    monitors: &[],
                    coordinate_ratio: 1.0,
                    minimum_size: Vec2::splat(1.0),
                    surface: Surface::Native,
                },
                &mut drag,
                || {
                    cursor_reads += 1;
                    Some(global.into())
                },
            )
        });
        output.textures_delta.clear();
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, ViewportCommand::StartDrag))
        );
        let positions: Vec<_> = commands
            .iter()
            .filter_map(|command| match command {
                ViewportCommand::OuterPosition(position) => Some(*position),
                _ => None,
            })
            .collect();
        if index == 2 {
            let moved = snip2pin::pin::host_frame(frame.moved_to(
                target + frame.image.min.to_vec2(),
                &[screen],
                1.0,
            ));
            assert_eq!(positions, [moved.window.min]);
            assert_eq!(moved.image.size(), frame.image.size());
        } else {
            assert!(positions.is_empty());
            assert!(
                !commands
                    .iter()
                    .any(|command| matches!(command, ViewportCommand::InnerSize(_)))
            );
        }
        if index == 4 {
            assert_eq!(cursor_reads, 0, "no idle cursor polling");
        }
    }
    assert!(!drag.drag.is_active());
    assert!(events.try_recv().is_err());
}

#[test]
fn toolbar_fits_tiny_images_without_changing_their_desktop_bounds() {
    for size in [[1.0, 1.0], [12.5, 7.5], [193.0, 121.0]] {
        for (ratio, grid) in [(1.0, 1.0), (0.8, 1.25), (1.0, 2.0)] {
            let placement = Placement {
                position: [-680.5, 100.25],
                size,
            };
            let frame = frame_geometry(
                placement,
                Rect::from_min_size(egui::pos2(-1000.0, -1000.0), egui::vec2(3000.0, 3000.0)),
                ratio,
                grid,
            );
            let image = frame.image;
            let (copy, close) = frame.toolbar_rects();
            let bounds = Rect::from_min_size(Pos2::ZERO, frame.window.size());
            assert!(bounds.contains_rect(copy) && bounds.contains_rect(close));
            assert!(copy.max.x < close.min.x);
            assert!(copy.max.y < image.min.y && close.max.y < image.min.y);
            assert!(
                (frame.window.min + image.min.to_vec2())
                    .distance(Pos2::new(placement.position[0], placement.position[1]) * ratio)
                    < 0.001
            );
            assert!((image.size() - egui::vec2(size[0], size[1]) * ratio).length() < 0.001);
        }
    }
}

#[test]
fn top_edge_layout_keeps_full_image_and_controls_visible() {
    for origin in [egui::pos2(0.0, 28.0), egui::pos2(-680.0, -172.0)] {
        for ratio in [1.0, 0.8] {
            for size in [[12.0, 8.0], [193.0, 121.0], [680.0, 420.0]] {
                let screen = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
                let placement = Placement {
                    position: [
                        origin.x + if size[0] < 680.0 { 80.0 } else { 0.0 },
                        origin.y - 28.0,
                    ],
                    size,
                };
                let frame = frame_geometry(placement, screen, ratio, 1.0 / ratio);
                let (copy, close) = frame.toolbar_rects();
                assert!(frame.top_edge);
                assert!(frame.window.min.y + frame.image.min.y >= screen.min.y * ratio);
                let bounds = Rect::from_min_size(Pos2::ZERO, frame.window.size());
                assert!(bounds.contains_rect(copy) && bounds.contains_rect(close));
                let visible = Rect::from_min_max(screen.min * ratio, screen.max * ratio);
                assert!(visible.contains_rect(copy.translate(frame.window.min.to_vec2())));
                assert!(visible.contains_rect(close.translate(frame.window.min.to_vec2())));
                assert!(!copy.intersects(frame.image) && !close.intersects(frame.image));
                assert_eq!(frame.toolbar_side, ToolbarSide::Bottom);
                assert!((frame.image.aspect_ratio() - size[0] / size[1]).abs() < 0.001);
                assert!(visible.contains_rect(frame.image.translate(frame.window.min.to_vec2())));
                if size[1] < screen.height() {
                    assert!(
                        (frame.image.size() - egui::vec2(size[0], size[1]) * ratio).length()
                            < 0.001
                    );
                } else {
                    assert!(frame.image.width() < size[0] * ratio);
                }
            }
        }
    }
}

#[test]
fn clicking_toolbar_buttons_dispatches_actions_without_window_drag() {
    for (copy, side) in [
        (true, ToolbarSide::Top),
        (false, ToolbarSide::Top),
        (true, ToolbarSide::Bottom),
        (false, ToolbarSide::Bottom),
        (true, ToolbarSide::Left),
        (false, ToolbarSide::Left),
        (true, ToolbarSide::Right),
        (false, ToolbarSide::Right),
    ] {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "pin",
            egui::ColorImage::filled([2, 2], Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let (sender, events) = mpsc::channel();
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(680.0, 420.0));
        let position = match side {
            ToolbarSide::Top => screen.min + Vec2::splat(100.0),
            ToolbarSide::Bottom => screen.min + egui::vec2(100.0, -50.0),
            ToolbarSide::Left => egui::pos2(screen.max.x - 50.0, screen.min.y + 100.0),
            ToolbarSide::Right => screen.min + egui::vec2(-50.0, 100.0),
        };
        let mut frame = frame_geometry(
            Placement {
                position: position.into(),
                size: [193.0, 121.0],
            },
            screen,
            1.0,
            1.0,
        );
        frame = frame.moved_to(position, &[screen], 1.0);
        assert_eq!(frame.toolbar_side, side);
        let (copy_rect, close_rect) = frame.toolbar_rects();
        let point = if copy {
            copy_rect.center()
        } else {
            close_rect.center()
        };
        for input_events in [
            vec![egui::Event::PointerMoved(point)],
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            let mut frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, frame.window.size())),
                    events: input_events,
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        Model {
                            id: 41,
                            texture: &texture,
                            frame,
                            events: &|event| {
                                let _ = sender.send(event);
                            },
                            monitors: &[],
                            coordinate_ratio: 1.0,
                            minimum_size: Vec2::splat(1.0),
                            surface: Surface::Native,
                        },
                        &mut Preparation::default(),
                        || panic!("toolbar must not read the desktop cursor"),
                    )
                },
            );
            frame.textures_delta.clear();
            assert!(
                frame.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .all(|command| !matches!(command, ViewportCommand::StartDrag))
            );
        }
        if copy {
            assert!(matches!(events.try_recv(), Ok(Event::CopyPin(41))));
        } else {
            assert!(matches!(events.try_recv(), Ok(Event::ClosePin(41))));
        }
        assert!(events.try_recv().is_err());
    }
}

#[test]
fn close_button_tracks_hover_and_pointer_leave_without_changing_image_bounds() {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "pin",
        egui::ColorImage::filled([2, 2], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    let (sender, _events) = mpsc::channel();
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(220.0, 140.0));
    let image_rect = Rect::from_min_size(egui::pos2(10.0, 42.0), egui::vec2(200.0, 88.0));
    let layout = Frame {
        window: bounds,
        image: image_rect,
        controls_area: bounds,
        grid: 1.0,
        toolbar_side: ToolbarSide::Top,
        toolbar_offset: Vec2::ZERO,
        top_edge: false,
    };
    for (events, close_visible) in [
        (vec![], false),
        (
            vec![egui::Event::PointerMoved(egui::pos2(80.0, 60.0))],
            true,
        ),
        (
            vec![egui::Event::PointerMoved(
                toolbar_rects(image_rect, false).0.center(),
            )],
            true,
        ),
        (vec![egui::Event::PointerGone], false),
    ] {
        let mut frame = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                events,
                ..Default::default()
            },
            |ui| {
                show(
                    ui,
                    Model {
                        id: 1,
                        texture: &texture,
                        frame: layout,
                        events: &|event| {
                            let _ = sender.send(event);
                        },
                        monitors: &[],
                        coordinate_ratio: 1.0,
                        minimum_size: Vec2::splat(1.0),
                        surface: Surface::Native,
                    },
                    &mut Preparation::default(),
                    || panic!("hover must not read the desktop cursor"),
                )
            },
        );
        frame.textures_delta.clear();
        assert_eq!(
            frame.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "×")
            }),
            close_visible
        );
        assert_eq!(
            frame.shapes.iter().any(
                |shape| matches!(&shape.shape, egui::Shape::Path(path) if path.points.len() == 5)
            ),
            close_visible
        );
        let image = frame
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                    Some(mesh.calc_bounds())
                }
                _ => None,
            })
            .expect("image is painted");
        assert_eq!(image, image_rect);
        assert!(
            frame.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Rect(rect) if rect.blur_width > 0.0)
            }),
            "the shadow is rendered outside the image"
        );
    }
}
