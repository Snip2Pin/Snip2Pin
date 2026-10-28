use super::*;
use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::NSPoint;

fn native_event(
    kind: NSEventType,
    window: isize,
    number: isize,
    point: NSPoint,
) -> Retained<NSEvent> {
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind, point, NSEventModifierFlags::Shift, 123.0,
        window, None, number, 1, 1.0,
    ).expect("synthetic event; never posted to AppKit")
}

struct Fixture {
    app: Snip2Pin,
    ctx: egui::Context,
    paint: BTreeMap<u64, Arc<egui::viewport::DeferredViewportUiCallback>>,
    size: egui::Vec2,
}

impl Fixture {
    fn new() -> Self {
        Self::with_size(egui::vec2(196.0, 137.0))
    }

    fn with_size(size: egui::Vec2) -> Self {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        let pin = &app.pins[&1];
        let second = Pin {
            zoom: platform::pin_zoom::PinZoom::new(2),
            lock_limits: None,
            name: String::new(),
            texture: pin.texture.clone(),
            pixels: pin.pixels.clone(),
            initial: pin.initial,
            display_scale: pin.display_scale,
            screen: pin.screen,
            screen_areas: pin.screen_areas.clone(),
            session: pin.session,
            first_frame_drawn: Arc::new(AtomicBool::new(false)),
            locked: Arc::new(AtomicBool::new(false)),
            show_requested: Cell::new(false),
        };
        app.pins.insert(2, second);
        let area = egui::Rect::from_min_max(egui::pos2(0.0, 30.0), egui::pos2(2560.0, 1440.0));
        for pin in app.pins.values_mut() {
            pin.screen = area;
            pin.screen_areas = Arc::new(vec![area]);
        }
        app.demo = false;
        let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
        setup.textures_delta.clear();
        let paint = app
            .pins
            .keys()
            .map(|&id| {
                (
                    id,
                    setup.viewport_output[&pin_id(id)]
                        .viewport_ui_cb
                        .clone()
                        .expect("pin UI"),
                )
            })
            .collect();
        Self {
            app,
            ctx,
            paint,
            size,
        }
    }

    fn observe(&self, kind: NSEventType, window: isize, number: isize, point: egui::Pos2) {
        self.app.pin_drag.record_for_test(
            native_event(
                kind,
                window,
                number,
                NSPoint::new(f64::from(point.x), f64::from(self.size.y - point.y)),
            ),
            if window == 33 { 2 } else { 1 },
            [f64::from(point.x) * 2.0, f64::from(point.y) * 2.0],
        );
    }

    fn paint(
        &self,
        id: u64,
        point: egui::Pos2,
        pressed: Option<bool>,
        multipass: bool,
    ) -> egui::FullOutput {
        let mut events = vec![egui::Event::PointerMoved(point)];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let mut input = egui::RawInput {
            viewport_id: pin_id(id),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            events,
            ..Default::default()
        };
        input.viewports.insert(
            pin_id(id),
            egui::ViewportInfo {
                native_pixels_per_point: Some(2.0),
                inner_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(1847.0, 62.0),
                    self.size,
                )),
                focused: Some(true),
                ..Default::default()
            },
        );
        self.ctx.run_ui(input, |ui| {
            (self.paint[&id])(ui);
            if multipass && ui.ctx().current_pass_index() == 0 {
                ui.ctx().request_discard("repeat native press frame");
            }
        })
    }

    fn requests(&self) -> Vec<(u64, platform::macos_drag::PressId)> {
        self.app
            .events
            .try_iter()
            .filter_map(|event| match event {
                Event::BeginPinDrag { pin_id: id, press } => Some((id, press)),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn first_press_keeps_its_native_event_across_multiple_ui_passes() {
    let fixture = Fixture::new();
    let point = egui::pos2(111.7, 74.1);
    fixture.paint(1, point, None, false).textures_delta.clear();
    fixture.observe(NSEventType::LeftMouseDown, 22, 42, point);
    let token = fixture
        .app
        .pin_drag
        .requests()
        .current()
        .expect("native down");
    let mut output = fixture.paint(1, point, Some(true), true);
    output.textures_delta.clear();
    assert!(
        output
            .viewport_output
            .values()
            .all(|viewport| !viewport.commands.contains(&ViewportCommand::StartDrag)),
        "deferred backend drag would use an unrelated NSApplication.currentEvent"
    );
    let requests = fixture.requests();
    assert_eq!(requests, [(1, token)]);
    let selected = fixture
        .app
        .pin_drag
        .take_for_test(requests[0].1, 22, true)
        .expect("original down");
    let location = selected.locationInWindow();
    assert!((location.x - 111.7).abs() < 0.001 && (location.y - 62.9).abs() < 0.001);
    assert_eq!(selected.eventNumber(), 42);
    assert_eq!(selected.timestamp(), 123.0);
    assert_eq!(selected.modifierFlags(), NSEventModifierFlags::Shift);
    assert!(
        fixture
            .app
            .pin_drag
            .take_for_test(token, 22, true)
            .is_none()
    );
}

#[test]
fn queued_body_drag_cannot_consume_a_later_copy_or_close_press() {
    for size in [
        egui::vec2(196.0, 137.0),
        egui::vec2(62.0, 120.0),
        egui::vec2(32.0, 120.0),
    ] {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        let row = pin::toolbar_geometry(bounds, &[], bounds, 1.0).expect("toolbar");
        for index in 0..row.count {
            let fixture = Fixture::with_size(size);
            let body = egui::pos2(size.x / 2.0, size.y - 20.0);
            fixture.paint(1, body, None, false).textures_delta.clear();
            fixture.observe(NSEventType::LeftMouseDown, 22, 1, body);
            fixture
                .paint(1, body, Some(true), false)
                .textures_delta
                .clear();
            let queued = fixture.requests();
            assert_eq!(queued.len(), 1);
            fixture.observe(NSEventType::LeftMouseUp, 22, 1, body);
            fixture
                .paint(1, body, Some(false), false)
                .textures_delta
                .clear();

            let step = pin::METRICS.button + pin::METRICS.button_gap;
            let button = row.bounds.min
                + egui::Vec2::splat(pin::METRICS.button / 2.0)
                + egui::vec2(index as f32 * step, 0.0);
            fixture.paint(1, button, None, false).textures_delta.clear();
            fixture.observe(NSEventType::LeftMouseDown, 22, 2, button);
            fixture
                .paint(1, button, Some(true), false)
                .textures_delta
                .clear();
            let current = fixture
                .app
                .pin_drag
                .requests()
                .current()
                .expect("button press");
            assert_ne!(queued[0].1, current);
            assert!(
                fixture.requests().is_empty(),
                "controls must not initiate a drag"
            );
            assert!(
                fixture
                    .app
                    .pin_drag
                    .take_for_test(queued[0].1, 22, true)
                    .is_none()
            );
            assert_eq!(fixture.app.pin_drag.requests().current(), Some(current));
            fixture.observe(NSEventType::LeftMouseUp, 22, 2, button);
            assert!(fixture.app.pin_drag.requests().current().is_none());
        }
    }
}

#[test]
fn an_old_pin_request_preserves_another_pins_current_drag() {
    let fixture = Fixture::new();
    let body = egui::pos2(90.0, 80.0);
    fixture.paint(1, body, None, false).textures_delta.clear();
    fixture.observe(NSEventType::LeftMouseDown, 22, 1, body);
    fixture
        .paint(1, body, Some(true), false)
        .textures_delta
        .clear();
    let old = fixture.requests();
    assert_eq!(old.len(), 1);
    fixture.observe(NSEventType::LeftMouseUp, 22, 1, body);
    fixture
        .paint(1, body, Some(false), false)
        .textures_delta
        .clear();
    fixture.paint(2, body, None, false).textures_delta.clear();
    fixture.observe(NSEventType::LeftMouseDown, 33, 2, body);
    fixture
        .paint(2, body, Some(true), true)
        .textures_delta
        .clear();
    let new = fixture.requests();
    assert_eq!(new.len(), 1);
    assert_eq!((old[0].0, new[0].0), (1, 2));
    assert_ne!(old[0].1, new[0].1);
    assert!(
        fixture
            .app
            .pin_drag
            .take_for_test(old[0].1, 22, true)
            .is_none()
    );
    let selected = fixture
        .app
        .pin_drag
        .take_for_test(new[0].1, 33, true)
        .expect("second pin press");
    assert_eq!(selected.eventNumber(), 2);
    assert_eq!(selected.windowNumber(), 33);
}

#[test]
fn missing_or_released_native_press_never_falls_back_to_current_event() {
    for released in [false, true] {
        let fixture = Fixture::new();
        let point = egui::pos2(90.0, 80.0);
        fixture.paint(1, point, None, false).textures_delta.clear();
        if released {
            fixture.observe(NSEventType::LeftMouseDown, 22, 1, point);
            fixture.observe(NSEventType::LeftMouseUp, 22, 1, point);
        }
        let mut output = fixture.paint(1, point, Some(true), true);
        output.textures_delta.clear();
        assert!(fixture.requests().is_empty());
        assert!(
            output
                .viewport_output
                .values()
                .all(|viewport| !viewport.commands.contains(&ViewportCommand::StartDrag))
        );
    }
}

#[test]
fn older_ui_input_cannot_adopt_a_newer_native_button_press() {
    let fixture = Fixture::new();
    let body = egui::pos2(90.0, 80.0);
    fixture.paint(1, body, None, false).textures_delta.clear();
    fixture.observe(NSEventType::LeftMouseDown, 22, 1, body);
    fixture.observe(NSEventType::LeftMouseUp, 22, 1, body);
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, fixture.size);
    let controls = pin::toolbar_geometry(bounds, &[], bounds, 1.0).expect("controls");
    let button = controls.bounds.center();
    fixture.observe(NSEventType::LeftMouseDown, 22, 2, button);
    let current = fixture.app.pin_drag.requests().current().expect("new down");
    // Replay an older RawInput while AppKit has already observed the next click.
    fixture
        .paint(1, body, Some(true), false)
        .textures_delta
        .clear();
    assert!(fixture.requests().is_empty());
    assert_eq!(fixture.app.pin_drag.requests().current(), Some(current));
}

#[test]
fn older_ui_input_cannot_adopt_another_windows_native_press() {
    let fixture = Fixture::new();
    let body = egui::pos2(90.0, 80.0);
    fixture.paint(1, body, None, false).textures_delta.clear();
    fixture.observe(NSEventType::LeftMouseDown, 33, 2, body);
    fixture
        .paint(1, body, Some(true), false)
        .textures_delta
        .clear();
    assert!(fixture.requests().is_empty());
    let current = fixture
        .app
        .pin_drag
        .requests()
        .current()
        .expect("second pin down");
    assert_eq!(
        fixture
            .app
            .pin_drag
            .take_for_test(current, 33, true)
            .expect("not consumed")
            .eventNumber(),
        2
    );
}
