use super::{gestures::Fixture, *};

fn key(key: egui::Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
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
    assert!(
        commands.is_empty(),
        "keyboard input only emits semantic movement"
    );
    actions
        .into_iter()
        .fold(Vec2::ZERO, |sum, action| match action {
            Action::Move(7, steps) => sum + egui::vec2(steps[0] as f32, steps[1] as f32),
            other => panic!("unexpected action: {other:?}"),
        })
}

#[test]
fn a_held_arrow_accelerates_without_system_repeat_events_and_stops_on_release() {
    let mut fixture = Fixture::new();
    assert_eq!(
        frame(
            &mut fixture,
            0.0,
            vec![key(egui::Key::ArrowRight, true)],
            true,
            false
        ),
        egui::vec2(1.0, 0.0)
    );
    let mut early = 0.0;
    let mut late = 0.0;
    let mut capped = 0.0;
    for i in 1..=180 {
        let delta = frame(&mut fixture, f64::from(i) / 60.0, vec![], true, false);
        assert_eq!(delta.y, 0.0);
        if i <= 12 {
            early += delta.x;
        }
        if (49..=60).contains(&i) {
            late += delta.x;
        }
        if i > 120 {
            capped += delta.x;
        }
    }
    assert!(
        early > 0.0 && late > early * 2.0,
        "held keys must accelerate: {early} -> {late}"
    );
    assert!(
        (capped - 900.0).abs() <= 1.0,
        "maximum native steps per second: {capped}"
    );
    let stalled = frame(&mut fixture, 100.0, vec![], true, false);
    assert!(
        stalled.x > 0.0 && stalled.x <= 91.0,
        "a stalled frame must not catch up: {stalled:?}"
    );
    assert_eq!(
        frame(
            &mut fixture,
            100.01,
            vec![key(egui::Key::ArrowRight, false)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(frame(&mut fixture, 101.0, vec![], true, false), Vec2::ZERO);
}

#[test]
fn a_held_arrow_stays_slow_before_accelerating() {
    let mut fixture = Fixture::new();
    let tap = frame(
        &mut fixture,
        0.0,
        vec![key(egui::Key::ArrowRight, true)],
        true,
        false,
    );
    assert_eq!(tap.x, 1.0);
    let mut held = 0.0;
    for i in 1..=24 {
        let delta = frame(&mut fixture, f64::from(i) / 60.0, vec![], true, false);
        assert!(delta.x <= 1.0, "fine adjustment jumped: {delta:?}");
        held += delta.x;
    }
    assert_eq!(held, 3.0, "first 0.4 seconds must remain slow");
    assert_eq!(
        frame(
            &mut fixture,
            0.41,
            vec![key(egui::Key::ArrowRight, false)],
            true,
            false,
        ),
        Vec2::ZERO
    );
    assert_eq!(frame(&mut fixture, 1.0, vec![], true, false), Vec2::ZERO);
}

#[test]
fn system_repeat_frequency_cannot_change_the_movement_speed() {
    let mut quiet = Fixture::new();
    let mut repeated = Fixture::new();
    for fixture in [&mut quiet, &mut repeated] {
        frame(
            fixture,
            0.0,
            vec![key(egui::Key::ArrowRight, true)],
            true,
            false,
        );
    }
    for i in 1..=90 {
        let time = f64::from(i) / 60.0;
        let mut repeat = key(egui::Key::ArrowRight, true);
        if let Event::Key { repeat, .. } = &mut repeat {
            *repeat = true;
        }
        assert_eq!(
            frame(&mut quiet, time, vec![], true, false),
            frame(&mut repeated, time, vec![repeat], true, false)
        );
    }
}

#[test]
fn losing_focus_or_locking_requires_a_new_press_before_movement_resumes() {
    for lock in [false, true] {
        let mut fixture = Fixture::new();
        frame(
            &mut fixture,
            0.0,
            vec![key(egui::Key::ArrowRight, true)],
            true,
            false,
        );
        for i in 1..=30 {
            frame(&mut fixture, f64::from(i) / 60.0, vec![], true, false);
        }
        assert_eq!(frame(&mut fixture, 0.6, vec![], lock, lock), Vec2::ZERO);
        assert_eq!(frame(&mut fixture, 0.7, vec![], true, false), Vec2::ZERO);
        frame(
            &mut fixture,
            0.8,
            vec![key(egui::Key::ArrowRight, false)],
            true,
            false,
        );
        assert_eq!(
            frame(
                &mut fixture,
                0.9,
                vec![key(egui::Key::ArrowRight, true)],
                true,
                false
            ),
            egui::vec2(1.0, 0.0)
        );
    }
}

fn travel(fps: u32, diagonal: bool) -> Vec2 {
    let mut fixture = Fixture::new();
    let mut events = vec![key(egui::Key::ArrowRight, true)];
    if diagonal {
        events.push(key(egui::Key::ArrowDown, true));
    }
    let mut total = frame(&mut fixture, 0.0, events, true, false);
    for i in 1..=fps * 3 {
        total += frame(
            &mut fixture,
            f64::from(i) / f64::from(fps),
            vec![],
            true,
            false,
        );
    }
    total
}

#[test]
fn elapsed_time_and_normalized_direction_keep_speed_consistent_across_frame_rates() {
    let reference = travel(60, false).length();
    for fps in [30, 60, 144] {
        for diagonal in [false, true] {
            assert!(
                (travel(fps, diagonal).length() - reference).abs() <= 2.0,
                "fps={fps}, diagonal={diagonal}"
            );
        }
    }
}

#[test]
fn shift_arrows_step_on_key_events_without_frame_driven_movement() {
    let mut fixture = Fixture::new();
    let mut press = key(egui::Key::ArrowRight, true);
    if let Event::Key { modifiers, .. } = &mut press {
        *modifiers = Modifiers::SHIFT;
    }
    assert_eq!(
        frame(
            &mut fixture,
            0.0,
            vec![Event::ModifiersChanged(Modifiers::SHIFT), press.clone()],
            true,
            false,
        ),
        egui::vec2(10.0, 0.0)
    );
    assert_eq!(frame(&mut fixture, 0.1, vec![], true, false), Vec2::ZERO);
    if let Event::Key { repeat, .. } = &mut press {
        *repeat = true;
    }
    assert_eq!(
        frame(&mut fixture, 0.2, vec![press], true, false),
        egui::vec2(10.0, 0.0)
    );
    assert_eq!(frame(&mut fixture, 0.3, vec![], true, false), Vec2::ZERO);
}

#[test]
fn shift_repeats_cannot_resume_movement_after_focus_loss_locking_or_cleanup() {
    for interruption in ["focus", "lock", "cleanup"] {
        let mut fixture = Fixture::new();
        let mut press = key(egui::Key::ArrowRight, true);
        if let Event::Key { modifiers, .. } = &mut press {
            *modifiers = Modifiers::SHIFT;
        }
        assert_eq!(
            frame(
                &mut fixture,
                0.0,
                vec![Event::ModifiersChanged(Modifiers::SHIFT), press.clone()],
                true,
                false
            ),
            egui::vec2(10.0, 0.0)
        );
        if interruption == "cleanup" {
            fixture.forget_input();
        }
        assert_eq!(
            frame(
                &mut fixture,
                0.1,
                vec![],
                interruption != "focus",
                interruption == "lock"
            ),
            Vec2::ZERO
        );
        if let Event::Key { repeat, .. } = &mut press {
            *repeat = true;
        }
        assert_eq!(
            frame(&mut fixture, 0.2, vec![press], true, false),
            Vec2::ZERO
        );
        assert_eq!(
            frame(
                &mut fixture,
                0.3,
                vec![key(egui::Key::ArrowRight, false)],
                true,
                false,
            ),
            Vec2::ZERO
        );
        assert_eq!(frame(&mut fixture, 1.0, vec![], true, false), Vec2::ZERO);
    }
}

#[test]
fn changing_shift_while_an_arrow_is_held_switches_between_direct_steps_and_fine_control() {
    let mut fixture = Fixture::new();
    frame(
        &mut fixture,
        0.0,
        vec![key(egui::Key::ArrowRight, true)],
        true,
        false,
    );
    for i in 1..=60 {
        frame(&mut fixture, f64::from(i) / 60.0, vec![], true, false);
    }
    assert_eq!(
        frame(
            &mut fixture,
            1.01,
            vec![Event::ModifiersChanged(Modifiers::SHIFT)],
            true,
            false,
        ),
        Vec2::ZERO,
        "a modifier change must not retroactively charge the previous frame"
    );
    assert_eq!(frame(&mut fixture, 1.03, vec![], true, false), Vec2::ZERO);
    let repeat = Event::Key {
        key: egui::Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: Modifiers::SHIFT,
    };
    assert_eq!(
        frame(&mut fixture, 1.06, vec![repeat], true, false),
        egui::vec2(10.0, 0.0)
    );
    frame(
        &mut fixture,
        1.07,
        vec![Event::ModifiersChanged(Modifiers::NONE)],
        true,
        false,
    );
    let mut fine = 0.0;
    for i in 1..=24 {
        let delta = frame(
            &mut fixture,
            1.07 + f64::from(i) / 60.0,
            vec![],
            true,
            false,
        );
        assert!(delta.x <= 1.0, "releasing Shift must restore fine control");
        fine += delta.x;
    }
    assert_eq!(fine, 3.0);
}

#[test]
fn opposite_arrows_stop_motion_and_release_resumes_at_the_initial_pace() {
    let mut fixture = Fixture::new();
    frame(
        &mut fixture,
        0.0,
        vec![key(egui::Key::ArrowRight, true)],
        true,
        false,
    );
    for i in 1..=60 {
        frame(&mut fixture, f64::from(i) / 60.0, vec![], true, false);
    }
    assert_eq!(
        frame(
            &mut fixture,
            1.01,
            vec![key(egui::Key::ArrowLeft, true)],
            true,
            false
        ),
        Vec2::ZERO
    );
    assert_eq!(frame(&mut fixture, 2.0, vec![], true, false), Vec2::ZERO);
    assert_eq!(
        frame(
            &mut fixture,
            2.01,
            vec![key(egui::Key::ArrowLeft, false)],
            true,
            false
        ),
        Vec2::ZERO
    );
    let mut resumed = 0.0;
    for i in 1..=24 {
        resumed += frame(
            &mut fixture,
            2.01 + f64::from(i) / 60.0,
            vec![],
            true,
            false,
        )
        .x;
    }
    assert_eq!(
        resumed, 3.0,
        "resumed direction must start at the fine pace"
    );
}

#[test]
fn reserved_modifiers_and_explicit_cleanup_cancel_a_hold() {
    for explicit in [false, true] {
        let mut fixture = Fixture::new();
        frame(
            &mut fixture,
            0.0,
            vec![key(egui::Key::ArrowRight, true)],
            true,
            false,
        );
        frame(&mut fixture, 0.05, vec![], true, false);
        let events = if explicit {
            fixture.forget_input();
            vec![]
        } else {
            vec![Event::ModifiersChanged(Modifiers::ALT)]
        };
        assert_eq!(frame(&mut fixture, 0.1, events, true, false), Vec2::ZERO);
        assert_eq!(
            frame(
                &mut fixture,
                0.2,
                vec![Event::ModifiersChanged(Modifiers::NONE)],
                true,
                false
            ),
            Vec2::ZERO
        );
    }
}
