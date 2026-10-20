use super::*;

struct Fixture {
    ctx: egui::Context,
    texture: TextureHandle,
    frame: u32,
}

impl Fixture {
    fn new() -> Self {
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
        let actions = RefCell::new(Vec::new());
        let commands = RefCell::new(Vec::new());
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 160.0))),
                events,
                time: Some(f64::from(self.frame) / 60.0),
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
        self.frame += 1;
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
fn trackpad_pan_is_direct_and_batches_both_axes_without_native_drag() {
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
    assert_eq!(actions, [Action::Pan(7, egui::vec2(4.25, -1.5))]);
    assert!(commands.is_empty());
    assert!(
        fixture
            .paint(Vec::new(), Interaction::default())
            .0
            .is_empty()
    );
}

#[test]
fn native_pinch_takes_priority_over_synthesized_scroll_and_pan() {
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
                vec![Action::Zoom(7, vec![Zoom::Scale(0.625)])]
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
                vec![Action::Zoom(7, vec![Zoom::Scale(1.2)])]
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
