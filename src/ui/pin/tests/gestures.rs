use super::*;

pub(super) struct Fixture {
    ctx: egui::Context,
    texture: TextureHandle,
    frame: u32,
}

impl Fixture {
    pub(super) fn forget_input(&self) {
        super::super::forget_input(&self.ctx, 7);
    }
    pub(super) fn new() -> Self {
        let ctx = egui::Context::default();
        let texture = texture(&ctx);
        Self {
            ctx,
            texture,
            frame: 0,
        }
    }

    fn paint(
        &mut self,
        events: Vec<Event>,
        interaction: Interaction,
    ) -> (Vec<Action>, Vec<ViewportCommand>) {
        let time = f64::from(self.frame) / 60.0;
        self.frame += 1;
        self.paint_at(time, events, interaction)
    }

    pub(super) fn paint_at(
        &mut self,
        time: f64,
        events: Vec<Event>,
        interaction: Interaction,
    ) -> (Vec<Action>, Vec<ViewportCommand>) {
        let actions = RefCell::new(Vec::new());
        let commands = RefCell::new(Vec::new());
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
                events,
                time: Some(time),
                ..Default::default()
            },
            |ui| {
                image_with_host(
                    ui,
                    7,
                    &self.texture,
                    interaction,
                    &|action| actions.borrow_mut().push(action),
                    |command| commands.borrow_mut().push(command),
                );
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("gesture effects occur once");
                }
            },
        );
        output.textures_delta.clear();
        (actions.into_inner(), commands.into_inner())
    }
}

fn wheel(unit: egui::MouseWheelUnit, delta: Vec2, modifiers: Modifiers) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        modifiers,
        phase: egui::TouchPhase::Move,
    }
}

#[test]
fn plain_precision_scroll_does_not_move_or_resize_the_pin() {
    let mut fixture = Fixture::new();
    let (actions, commands) = fixture.paint(
        vec![
            Event::PointerMoved(egui::pos2(80.0, 80.0)),
            wheel(
                egui::MouseWheelUnit::Point,
                egui::vec2(1.25, -2.5),
                Modifiers::NONE,
            ),
            wheel(
                egui::MouseWheelUnit::Point,
                egui::vec2(3.0, 1.0),
                Modifiers::SHIFT,
            ),
        ],
        Interaction::default(),
    );
    assert!(
        actions.is_empty(),
        "two-finger scrolling is not window dragging"
    );
    assert!(commands.is_empty());
    assert!(
        fixture
            .paint(Vec::new(), Interaction::default())
            .0
            .is_empty()
    );
}

#[test]
fn wheel_zoom_keeps_its_initial_anchor_until_the_native_end() {
    let mut fixture = Fixture::new();
    fixture.paint(
        vec![
            Event::PointerMoved(egui::pos2(80.0, 80.0)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::ZERO,
                modifiers: Modifiers::NONE,
                phase: egui::TouchPhase::Start,
            },
        ],
        Interaction::default(),
    );
    let (actions, _) = fixture.paint(
        vec![
            Event::PointerMoved(egui::pos2(-40.0, 80.0)),
            wheel(
                egui::MouseWheelUnit::Line,
                egui::vec2(0.0, 1.0),
                Modifiers::NONE,
            ),
        ],
        Interaction::default(),
    );
    assert!(matches!(actions.as_slice(), [Action::Zoom(7, commands)]
        if matches!(commands.as_slice(), [Zoom::At { anchor, .. }]
            if anchor.fraction == [1.0 / 3.0, 0.5])));
    fixture.paint(
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::ZERO,
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::End,
        }],
        Interaction::default(),
    );
    assert!(
        fixture
            .paint(
                vec![wheel(
                    egui::MouseWheelUnit::Line,
                    egui::vec2(0.0, 1.0),
                    Modifiers::NONE
                )],
                Interaction::default()
            )
            .0
            .is_empty()
    );
}

#[test]
fn zoom_keeps_its_original_image_anchor_when_the_pointer_leaves() {
    for native in [false, true] {
        let mut fixture = Fixture::new();
        let zoom = || {
            if native {
                Event::Zoom(1.1)
            } else {
                wheel(
                    egui::MouseWheelUnit::Line,
                    egui::vec2(0.0, 1.0),
                    Modifiers::NONE,
                )
            }
        };
        fixture.paint(
            vec![Event::PointerMoved(egui::pos2(80.0, 80.0)), zoom()],
            Interaction::default(),
        );
        let (actions, _) = fixture.paint(
            vec![Event::PointerMoved(egui::pos2(-40.0, 80.0)), zoom()],
            Interaction::default(),
        );
        assert!(
            matches!(actions.as_slice(), [Action::Zoom(7, commands)] if matches!(commands.as_slice(),
            [Zoom::At { anchor, .. }] if anchor.fraction == [1.0 / 3.0, 0.5]))
        );
        fixture.frame += 60;
        assert!(
            fixture
                .paint(vec![zoom()], Interaction::default())
                .0
                .is_empty(),
            "a later independent episode cannot reuse the previous target"
        );
    }
}

#[test]
fn wheel_zoom_started_outside_cannot_pick_up_a_pin_mid_gesture() {
    let mut fixture = Fixture::new();
    fixture.paint(
        vec![
            Event::PointerMoved(egui::pos2(-40.0, 80.0)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::ZERO,
                modifiers: Modifiers::NONE,
                phase: egui::TouchPhase::Start,
            },
        ],
        Interaction::default(),
    );
    assert!(
        fixture
            .paint(
                vec![
                    Event::PointerMoved(egui::pos2(80.0, 80.0)),
                    wheel(
                        egui::MouseWheelUnit::Line,
                        egui::vec2(0.0, 1.0),
                        Modifiers::NONE
                    )
                ],
                Interaction::default()
            )
            .0
            .is_empty()
    );
}

#[test]
fn native_end_and_cancel_release_the_target_even_with_reserved_modifiers() {
    for phase in [egui::TouchPhase::End, egui::TouchPhase::Cancel] {
        let mut fixture = Fixture::new();
        fixture.paint(
            vec![
                Event::PointerMoved(egui::pos2(80.0, 80.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::ZERO,
                    modifiers: Modifiers::CTRL,
                    phase: egui::TouchPhase::Start,
                },
            ],
            Interaction::default(),
        );
        fixture.paint(
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::ZERO,
                modifiers: Modifiers::ALT,
                phase,
            }],
            Interaction::default(),
        );
        assert!(
            fixture
                .paint(
                    vec![
                        Event::PointerMoved(egui::pos2(-40.0, 80.0)),
                        Event::ModifiersChanged(Modifiers::NONE),
                        Event::Zoom(1.1)
                    ],
                    Interaction::default()
                )
                .0
                .is_empty()
        );
    }
}

#[test]
fn zoom_continuation_delivered_to_another_app_viewport_keeps_the_original_pin() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let texture = texture(&ctx);
    let first = egui::ViewportId::from_hash_of("first");
    let second = egui::ViewportId::from_hash_of("second");
    let mut frame = 0;
    let mut paint = |viewport, id, events| {
        let actions = RefCell::new(Vec::new());
        let mut raw = egui::RawInput {
            viewport_id: viewport,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
            events,
            time: Some(f64::from(frame) / 60.0),
            ..Default::default()
        };
        for viewport in [first, second] {
            raw.viewports
                .entry(viewport)
                .or_default()
                .native_pixels_per_point = Some(2.0);
        }
        let mut output = ctx.run_ui(raw, |ui| {
            super::super::route_gesture(ui, &|action| actions.borrow_mut().push(action));
            image_with_host(
                ui,
                id,
                &texture,
                Interaction::default(),
                &|action| actions.borrow_mut().push(action),
                |_| {},
            );
        });
        output.textures_delta.clear();
        frame += 1;
        actions.into_inner()
    };
    paint(
        first,
        7,
        vec![
            Event::PointerMoved(egui::pos2(80.0, 80.0)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::ZERO,
                modifiers: Modifiers::NONE,
                phase: egui::TouchPhase::Start,
            },
        ],
    );
    let continued = paint(
        second,
        8,
        vec![
            Event::PointerMoved(egui::pos2(80.0, 80.0)),
            wheel(
                egui::MouseWheelUnit::Line,
                egui::vec2(0.0, 1.0),
                Modifiers::NONE,
            ),
        ],
    );
    assert!(matches!(continued.as_slice(), [Action::Zoom(7, _)]));
    paint(
        second,
        8,
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::ZERO,
            modifiers: Modifiers::ALT,
            phase: egui::TouchPhase::End,
        }],
    );
    let next = paint(
        second,
        8,
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Start,
        }],
    );
    assert!(matches!(next.as_slice(), [Action::Zoom(8, _)]));
}

#[test]
fn native_pinch_takes_priority_over_synthesized_scroll() {
    for locked in [false, true] {
        let mut fixture = Fixture::new();
        let (actions, commands) = fixture.paint(
            vec![
                Event::PointerMoved(egui::pos2(80.0, 80.0)),
                Event::Zoom(1.25),
                Event::Zoom(0.5),
                wheel(
                    egui::MouseWheelUnit::Point,
                    egui::vec2(12.0, 24.0),
                    Modifiers::CTRL,
                ),
                wheel(
                    egui::MouseWheelUnit::Point,
                    egui::vec2(3.0, 5.0),
                    Modifiers::NONE,
                ),
            ],
            Interaction {
                locked,
                ..Default::default()
            },
        );
        assert_eq!(
            actions,
            if locked {
                Vec::new()
            } else {
                vec![Action::Zoom(
                    7,
                    vec![Zoom::At {
                        factor: 0.625,
                        anchor: snip2pin::pin::ZoomAnchor {
                            fraction: [1.0 / 3.0, 0.5],
                            screen: None,
                        },
                    }],
                )]
            }
        );
        assert!(commands.is_empty());
    }
}

#[test]
fn pinch_ignores_invalid_factors_and_excluded_targets() {
    for excluded in [false, true] {
        let mut fixture = Fixture::new();
        let point = egui::pos2(80.0, 80.0);
        let (actions, _) = fixture.paint(
            vec![
                Event::PointerMoved(point),
                Event::Zoom(f32::NAN),
                Event::Zoom(f32::INFINITY),
                Event::Zoom(0.0),
                Event::Zoom(-1.0),
                Event::Zoom(1.2),
            ],
            Interaction {
                excluded: excluded.then_some(Rect::from_center_size(point, Vec2::splat(20.0))),
                ..Default::default()
            },
        );
        assert_eq!(
            actions,
            if excluded {
                Vec::new()
            } else {
                vec![Action::Zoom(
                    7,
                    vec![Zoom::At {
                        factor: 1.2,
                        anchor: snip2pin::pin::ZoomAnchor {
                            fraction: [1.0 / 3.0, 0.5],
                            screen: None,
                        },
                    }],
                )]
            }
        );
    }
}

#[test]
fn touchscreen_single_finger_requests_native_drag_and_locked_pins_only_hint() {
    for locked in [false, true] {
        let mut fixture = Fixture::new();
        let point = egui::pos2(80.0, 80.0);
        fixture.paint(vec![Event::PointerMoved(point)], Interaction::default());
        let (actions, commands) = fixture.paint(
            vec![
                Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::Start,
                    pos: point,
                    force: None,
                },
                Event::PointerMoved(point),
                pointer(point, true),
            ],
            Interaction {
                locked,
                ..Default::default()
            },
        );
        if locked {
            assert_eq!(actions, [Action::UnlockHint(7)]);
            assert!(commands.is_empty());
        } else {
            assert!(actions.is_empty());
            assert!(!commands.is_empty());
            assert!(
                commands
                    .iter()
                    .all(|command| matches!(command, ViewportCommand::StartDrag))
            );
        }
    }
}

#[test]
fn pinch_rejects_command_and_alt_shortcuts() {
    for modifiers in [Modifiers::COMMAND, Modifiers::ALT, Modifiers::MAC_CMD] {
        let mut fixture = Fixture::new();
        let (actions, commands) = fixture.paint(
            vec![
                Event::PointerMoved(egui::pos2(80.0, 80.0)),
                Event::ModifiersChanged(modifiers),
                Event::Zoom(1.25),
            ],
            Interaction::default(),
        );
        assert!(actions.is_empty());
        assert!(commands.is_empty());
    }
}
