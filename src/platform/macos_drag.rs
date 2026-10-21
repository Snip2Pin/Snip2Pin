//! Keep the initiating AppKit press until its native drag request is handled.
use std::{
    cell::RefCell,
    ptr::NonNull,
    rc::Rc,
    sync::{Arc, Mutex},
};

use block2::RcBlock;
use eframe::egui;
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PressId(u64);

#[derive(Clone, Copy)]
struct PressLocation {
    pin: u64,
    physical: [f64; 2],
}

#[derive(Clone, Copy)]
struct PressSnapshot {
    id: PressId,
    location: PressLocation,
}

/// Sendable metadata for deferred egui callbacks; native events stay on the UI thread.
#[derive(Clone, Default)]
pub struct DragRequests(Arc<Mutex<Option<PressSnapshot>>>);

impl DragRequests {
    pub fn matching(&self, pin: u64, origin: egui::Pos2, pixels_per_point: f32) -> Option<PressId> {
        let snapshot = (*self.0.lock().ok()?)?;
        // Match egui-winit's physical f64 -> f32 -> UI-point conversion exactly.
        // This validates input identity; it never calculates a window position.
        let point = egui::pos2(
            snapshot.location.physical[0] as f32 / pixels_per_point,
            snapshot.location.physical[1] as f32 / pixels_per_point,
        );
        (snapshot.location.pin == pin && point == origin).then_some(snapshot.id)
    }

    #[cfg(test)]
    pub(crate) fn current(&self) -> Option<PressId> {
        (*self.0.lock().ok()?).map(|snapshot| snapshot.id)
    }
}

#[derive(Default)]
struct PendingPress {
    generation: u64,
    press: Option<(PressSnapshot, Retained<NSEvent>)>,
    requests: DragRequests,
}

impl PendingPress {
    fn record(&mut self, event: Retained<NSEvent>, location: Option<PressLocation>) {
        match event.r#type() {
            NSEventType::LeftMouseDown => {
                // A new Down invalidates the previous press, including non-pin clicks.
                self.generation = self.generation.wrapping_add(1).max(1);
                self.press = location.map(|location| {
                    (
                        PressSnapshot {
                            id: PressId(self.generation),
                            location,
                        },
                        event,
                    )
                });
            }
            NSEventType::LeftMouseUp => self.press = None,
            _ => {}
        }
        self.publish();
    }

    fn publish(&self) {
        if let Ok(mut snapshot) = self.requests.0.lock() {
            *snapshot = self.press.as_ref().map(|(snapshot, _)| *snapshot);
        }
    }

    fn take(&mut self, id: PressId, window: isize, held: bool) -> Option<Retained<NSEvent>> {
        let (current, event) = self.press.as_ref()?;
        if current.id != id || event.windowNumber() != window {
            // An older request must not consume a newer click or another pin's press.
            return None;
        }
        let (_, event) = self.press.take()?;
        self.publish();
        held.then_some(event)
    }
}

/// Application-local observation, disabled by default for the GUI sandbox.
/// Rc keeps this owner and its retained events on their creating thread.
#[derive(Default)]
pub struct DragEvents {
    pending: Rc<RefCell<PendingPress>>,
    monitor: Option<Retained<AnyObject>>,
}

impl DragEvents {
    #[allow(unsafe_code)]
    pub fn install() -> Result<Self, snip2pin::i18n::Message> {
        let main = MainThreadMarker::new().ok_or(snip2pin::i18n::Text::UiThread)?;
        let mut events = Self::default();
        let pending = events.pending.clone();
        let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit owns this event during the synchronous callback.
            // Retaining it preserves the original press after currentEvent changes.
            if let Some(event) = unsafe { Retained::retain(event.as_ptr()) } {
                observe(&pending, event, main);
            }
            event.as_ptr()
        });
        // SAFETY: The handler returns the unmodified, valid incoming pointer.
        // AppKit retains the block; this owner removes the monitor on drop.
        events.monitor = Some(
            unsafe {
                NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                    NSEventMask::LeftMouseDown | NSEventMask::LeftMouseUp,
                    &handler,
                )
            }
            .ok_or(snip2pin::i18n::Text::NativeDragError)?,
        );
        Ok(events)
    }

    pub fn requests(&self) -> DragRequests {
        self.pending.borrow().requests.clone()
    }

    fn take(&self, id: PressId, window: isize, held: bool) -> Option<Retained<NSEvent>> {
        self.pending.borrow_mut().take(id, window, held)
    }

    pub fn begin(&self, pin: u64, press: PressId) {
        let Ok(window) = super::pin_window::find(pin) else {
            return;
        };
        if !window.isKeyWindow() {
            return;
        }
        let Some(event) = self.take(
            press,
            window.windowNumber(),
            NSEvent::pressedMouseButtons() & 1 != 0,
        ) else {
            return;
        };
        // No RefCell borrow survives AppKit's nested drag loop. AppKit owns all
        // movement; the exact original press supplies its mouse grab point.
        window.performWindowDragWithEvent(&event);
    }

    #[cfg(test)]
    pub(crate) fn record_for_test(&self, event: Retained<NSEvent>, pin: u64, physical: [f64; 2]) {
        self.pending
            .borrow_mut()
            .record(event, Some(PressLocation { pin, physical }));
    }

    #[cfg(test)]
    pub(crate) fn take_for_test(
        &self,
        id: PressId,
        window: isize,
        held: bool,
    ) -> Option<Retained<NSEvent>> {
        self.take(id, window, held)
    }

    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "Used by the separately compiled native lifecycle test"
    )]
    pub(crate) fn record_native_for_test(&self, event: Retained<NSEvent>) {
        observe(
            &self.pending,
            event,
            MainThreadMarker::new().expect("native test UI thread"),
        );
    }
}

fn observe(pending: &RefCell<PendingPress>, event: Retained<NSEvent>, main: MainThreadMarker) {
    let location = if event.r#type() == NSEventType::LeftMouseDown {
        press_location(&event, main)
    } else {
        None
    };
    pending.borrow_mut().record(event, location);
}

fn press_location(event: &NSEvent, main: MainThreadMarker) -> Option<PressLocation> {
    let window = event.window(main)?;
    let pin = super::pin_window::id_from_title(&window.title().to_string())?;
    // WinitView uses this same AppKit conversion, including its flipped origin.
    let point = window
        .contentView()?
        .convertPoint_fromView(event.locationInWindow(), None);
    let scale = window.backingScaleFactor();
    Some(PressLocation {
        pin,
        physical: [point.x * scale, point.y * scale],
    })
}

impl Drop for DragEvents {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if let Some(monitor) = &self.monitor {
            // SAFETY: Only install() creates this token on the main thread.
            // Rc prevents moving the owner away from that thread.
            unsafe { NSEvent::removeMonitor(monitor) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_app_kit::NSEventModifierFlags;
    use objc2_foundation::NSPoint;

    fn record(drag: &DragEvents, event: Retained<NSEvent>) {
        drag.record_for_test(event, 1, [223.4, 148.2]);
    }

    fn event(kind: NSEventType, window: isize, number: isize) -> Retained<NSEvent> {
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind, NSPoint::new(111.7, 62.9), NSEventModifierFlags::Shift, 123.0,
            window, None, number, 1, 1.0,
        ).expect("synthetic event; never posted to AppKit")
    }

    #[test]
    fn native_drag_retains_the_original_press_after_current_event_changes() {
        let drag = DragEvents::default();
        let original = event(NSEventType::LeftMouseDown, 22, 42);
        record(&drag, original.clone());
        let token = drag.requests().current().expect("press");
        let unrelated = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::AppKitDefined, NSPoint::new(0.0, 137.0),
            NSEventModifierFlags::empty(), 124.0, 22, None, 0, 0, 0,
        ).expect("synthetic AppKit event; never posted");
        record(&drag, unrelated);
        let selected = drag.take(token, 22, true).expect("original press");
        assert!(std::ptr::eq(&*selected, &*original));
        assert_eq!(selected.locationInWindow(), NSPoint::new(111.7, 62.9));
        assert_eq!(selected.eventNumber(), 42);
        assert_eq!(selected.timestamp(), 123.0);
        assert_eq!(selected.modifierFlags(), NSEventModifierFlags::Shift);
        assert!(drag.take(token, 22, true).is_none());
        assert!(drag.requests().current().is_none());
    }

    #[test]
    fn stale_or_wrong_window_requests_preserve_the_latest_press() {
        for latest_window in [22, 33] {
            let drag = DragEvents::default();
            record(&drag, event(NSEventType::LeftMouseDown, 22, 1));
            let old = drag.requests().current().expect("old");
            record(&drag, event(NSEventType::LeftMouseUp, 22, 1));
            record(&drag, event(NSEventType::LeftMouseDown, latest_window, 2));
            let current = drag.requests().current().expect("current");
            assert_ne!(old, current);
            assert!(drag.take(old, 22, true).is_none());
            assert!(drag.take(current, 999, true).is_none());
            assert_eq!(drag.requests().current(), Some(current));
            assert_eq!(
                drag.take(current, latest_window, true)
                    .expect("latest")
                    .eventNumber(),
                2
            );
        }
    }

    #[test]
    fn released_buttons_never_start_a_drag() {
        for delivered_up in [false, true] {
            let drag = DragEvents::default();
            record(&drag, event(NSEventType::LeftMouseDown, 22, 1));
            let token = drag.requests().current().expect("press");
            if delivered_up {
                record(&drag, event(NSEventType::LeftMouseUp, 22, 1));
            }
            assert!(drag.take(token, 22, delivered_up).is_none());
            assert!(drag.requests().current().is_none());
        }
    }

    #[test]
    fn a_press_during_native_handoff_survives_the_previous_request() {
        let drag = DragEvents::default();
        record(&drag, event(NSEventType::LeftMouseDown, 22, 1));
        let old = drag.requests().current().expect("first press");
        let original = drag.take(old, 22, true).expect("first handoff");
        assert!(drag.requests().current().is_none());
        // AppKit may run nested event delivery during performWindowDragWithEvent.
        record(&drag, event(NSEventType::LeftMouseUp, 22, 1));
        record(&drag, event(NSEventType::LeftMouseDown, 22, 2));
        let current = drag.requests().current().expect("new press");
        drop(original);
        assert!(drag.take(old, 22, true).is_none());
        assert_eq!(drag.requests().current(), Some(current));
        assert_eq!(
            drag.take(current, 22, true)
                .expect("new handoff")
                .eventNumber(),
            2
        );
    }

    #[test]
    fn a_new_non_pin_click_invalidates_a_previous_pin_press() {
        let drag = DragEvents::default();
        record(&drag, event(NSEventType::LeftMouseDown, 22, 1));
        let old = drag.requests().current().expect("old down");
        drag.pending
            .borrow_mut()
            .record(event(NSEventType::LeftMouseDown, 33, 2), None);
        assert!(drag.requests().current().is_none());
        assert!(drag.take(old, 22, true).is_none());
        record(&drag, event(NSEventType::LeftMouseDown, 22, 3));
        assert_ne!(drag.requests().current(), Some(old));
    }
}
