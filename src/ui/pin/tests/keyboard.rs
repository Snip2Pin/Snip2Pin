use super::{gestures::Fixture, *};

fn key(key: egui::Key, pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers,
    }
}

fn right(repeat: bool, modifiers: Modifiers) -> Event {
    key(egui::Key::ArrowRight, true, repeat, modifiers)
}

fn frame(
    fixture: &mut Fixture,
    time: f64,
    events: Vec<Event>,
    focused: bool,
    locked: bool,
) -> Vec2 {
    let (actions, commands) = fixture.paint_at(
        time,
        events,
        Interaction {
            keyboard_focus: Some(focused),
            locked,
            ..Default::default()
        },
    );
    assert!(commands.is_empty(), "only semantic movement is emitted");
    actions
        .into_iter()
        .fold(Vec2::ZERO, |sum, action| match action {
            Action::Move(7, steps) => sum + egui::vec2(steps[0] as f32, steps[1] as f32),
            other => panic!("unexpected action: {other:?}"),
        })
}

#[test]
fn a_short_tap_moves_once_even_with_intermediate_paints_and_multiple_passes() {
    for released_at in [0.12, 0.25] {
        let mut fixture = Fixture::new();
        assert_eq!(
            frame(
                &mut fixture,
                0.0,
                vec![right(false, Modifiers::NONE)],
                true,
                false
            ),
            egui::vec2(1.0, 0.0)
        );
        for i in 1..=10 {
            assert_eq!(
                frame(
                    &mut fixture,
                    released_at * f64::from(i) / 11.0,
                    vec![],
                    true,
                    false
                ),
                Vec2::ZERO,
                "redraws cannot synthesize more arrow presses"
            );
        }
        assert_eq!(
            frame(
                &mut fixture,
                released_at,
                vec![key(egui::Key::ArrowRight, false, false, Modifiers::NONE)],
                true,
                false
            ),
            Vec2::ZERO
        );
    }
}

#[test]
fn normal_holds_increase_step_size_only_on_system_repeat_events_and_cap_at_fast_steps() {
    let mut fixture = Fixture::new();
    frame(
        &mut fixture,
        0.0,
        vec![right(false, Modifiers::NONE)],
        true,
        false,
    );
    let mut previous = 1.0;
    for time in [0.2, 0.4, 0.6, 1.0, 2.0, 5.0, 10.0] {
        assert_eq!(
            frame(&mut fixture, time - 0.01, vec![], true, false),
            Vec2::ZERO
        );
        let step = frame(
            &mut fixture,
            time,
            vec![right(true, Modifiers::NONE)],
            true,
            false,
        )
        .x;
        assert!(
            (previous..=50.0).contains(&step),
            "invalid progression: {previous} -> {step}"
        );
        if time <= 0.4 {
            assert_eq!(step, 1.0, "initial repeats preserve fine adjustment");
        }
        previous = step;
    }
    assert_eq!(previous, 50.0);
    assert_eq!(frame(&mut fixture, 100.0, vec![], true, false), Vec2::ZERO);
}

#[test]
fn shift_arrows_move_fifty_steps_per_press_or_repeat_without_frame_motion() {
    let mut fixture = Fixture::new();
    assert_eq!(
        frame(
            &mut fixture,
            0.0,
            vec![
                Event::ModifiersChanged(Modifiers::SHIFT),
                right(false, Modifiers::SHIFT)
            ],
            true,
            false
        ),
        egui::vec2(50.0, 0.0)
    );
    for time in [0.1, 0.5, 1.0] {
        assert_eq!(frame(&mut fixture, time, vec![], true, false), Vec2::ZERO);
        assert_eq!(
            frame(
                &mut fixture,
                time + 0.01,
                vec![right(true, Modifiers::SHIFT)],
                true,
                false
            ),
            egui::vec2(50.0, 0.0)
        );
    }
}

#[test]
fn shift_changes_reset_normal_progress_without_an_extra_movement() {
    let mut fixture = Fixture::new();
    frame(
        &mut fixture,
        0.0,
        vec![right(false, Modifiers::NONE)],
        true,
        false,
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.0,
            vec![right(true, Modifiers::NONE)],
            true,
            false
        )
        .x,
        50.0
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.1,
            vec![Event::ModifiersChanged(Modifiers::SHIFT)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.2,
            vec![right(true, Modifiers::SHIFT)],
            true,
            false
        )
        .x,
        50.0
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.3,
            vec![Event::ModifiersChanged(Modifiers::NONE)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.4,
            vec![right(true, Modifiers::NONE)],
            true,
            false
        )
        .x,
        1.0
    );
}

#[test]
fn changing_or_opposing_directions_resets_the_repeat_step() {
    let mut fixture = Fixture::new();
    frame(
        &mut fixture,
        0.0,
        vec![right(false, Modifiers::NONE)],
        true,
        false,
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.0,
            vec![right(true, Modifiers::NONE)],
            true,
            false
        )
        .x,
        50.0
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.1,
            vec![key(egui::Key::ArrowLeft, true, false, Modifiers::NONE)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.2,
            vec![right(true, Modifiers::NONE)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.3,
            vec![key(egui::Key::ArrowLeft, false, false, Modifiers::NONE)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(
        frame(
            &mut fixture,
            5.4,
            vec![right(true, Modifiers::NONE)],
            true,
            false
        )
        .x,
        1.0
    );
}

#[test]
fn interrupted_repeats_cannot_restart_motion_without_a_fresh_press() {
    for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
        for interruption in ["focus", "lock", "cleanup", "reserved", "pointer"] {
            let mut fixture = Fixture::new();
            frame(
                &mut fixture,
                0.0,
                vec![Event::ModifiersChanged(modifiers), right(false, modifiers)],
                true,
                false,
            );
            if interruption == "cleanup" {
                fixture.forget_input();
            }
            let events = match interruption {
                "reserved" => vec![Event::ModifiersChanged(Modifiers::ALT)],
                "pointer" => vec![pointer(egui::pos2(80.0, 80.0), true)],
                _ => vec![],
            };
            fixture.paint_at(
                0.1,
                events,
                Interaction {
                    keyboard_focus: Some(interruption != "focus"),
                    locked: interruption == "lock",
                    ..Default::default()
                },
            );
            assert_eq!(
                frame(
                    &mut fixture,
                    0.2,
                    vec![
                        Event::ModifiersChanged(modifiers),
                        pointer(egui::pos2(80.0, 80.0), false),
                        right(true, modifiers)
                    ],
                    true,
                    false
                ),
                Vec2::ZERO,
                "{interruption} must require a fresh press"
            );
            frame(
                &mut fixture,
                0.3,
                vec![key(egui::Key::ArrowRight, false, false, modifiers)],
                true,
                false,
            );
            assert_eq!(
                frame(
                    &mut fixture,
                    0.4,
                    vec![right(false, modifiers)],
                    true,
                    false
                )
                .x,
                if modifiers.shift { 50.0 } else { 1.0 }
            );
        }
    }
}

#[test]
fn a_press_and_release_in_one_frame_still_moves_exactly_one_step() {
    let mut fixture = Fixture::new();
    assert_eq!(
        frame(
            &mut fixture,
            0.0,
            vec![
                right(false, Modifiers::NONE),
                key(egui::Key::ArrowRight, false, false, Modifiers::NONE)
            ],
            true,
            false
        ),
        egui::vec2(1.0, 0.0)
    );
    assert_eq!(frame(&mut fixture, 10.0, vec![], true, false), Vec2::ZERO);
}

#[test]
fn batched_key_events_keep_their_modifier_semantics_and_dispatch_once() {
    let mut fixture = Fixture::new();
    let delta = frame(
        &mut fixture,
        0.0,
        vec![
            Event::ModifiersChanged(Modifiers::SHIFT),
            right(false, Modifiers::SHIFT),
            right(true, Modifiers::SHIFT),
            Event::ModifiersChanged(Modifiers::NONE),
            right(true, Modifiers::NONE),
        ],
        true,
        false,
    );
    assert_eq!(delta, egui::vec2(101.0, 0.0));
    assert_eq!(frame(&mut fixture, 0.1, vec![], true, false), Vec2::ZERO);
}
