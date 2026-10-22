//! Discrete arrow-key input; system key repeats determine the movement cadence.
use eframe::egui::{self, Event, Key};
use snip2pin::pin::FAST_MOVE_STEPS;

use super::{Action, input::reserved};

// Preserve fine adjustment before increasing the distance of each repeat.
const FINE_HOLD_SECONDS: f64 = 0.4;
const STEP_GROWTH_PER_SECOND: f64 = 25.0;

#[derive(Clone, Copy)]
struct Hold {
    direction: [i32; 2],
    fast: bool,
    started: f64,
}

impl Hold {
    fn stride(self, time: f64) -> i32 {
        if self.fast {
            FAST_MOVE_STEPS
        } else {
            let accelerating = (time - self.started - FINE_HOLD_SECONDS).max(0.0);
            (1.0 + (accelerating * STEP_GROWTH_PER_SECOND).floor()).min(f64::from(FAST_MOVE_STEPS))
                as i32
        }
    }
}

fn state_id(id: u64) -> egui::Id {
    egui::Id::unique(("pin-held-arrows", id))
}

pub(super) fn forget(ctx: &egui::Context, id: u64) {
    ctx.data_mut(|data| data.remove::<Hold>(state_id(id)));
}

fn direction(key: Key) -> Option<[i32; 2]> {
    match key {
        Key::ArrowLeft => Some([-1, 0]),
        Key::ArrowRight => Some([1, 0]),
        Key::ArrowUp => Some([0, -1]),
        Key::ArrowDown => Some([0, 1]),
        _ => None,
    }
}

pub(super) fn adjust(ui: &egui::Ui, id: u64, focused: bool, emit: &dyn Fn(Action)) {
    let ctx = ui.ctx();
    let mut hold = ctx.data(|data| data.get_temp::<Hold>(state_id(id)));
    let steps = ui.input(|input| {
        if !focused
            || !input.time.is_finite()
            || reserved(input.modifiers)
            || input.pointer.any_down()
            || input
                .events
                .iter()
                .any(|event| matches!(event, Event::WindowFocused(false)))
        {
            hold = None;
            return [0, 0];
        }
        let held_direction = [
            i32::from(input.key_down(Key::ArrowRight)) - i32::from(input.key_down(Key::ArrowLeft)),
            i32::from(input.key_down(Key::ArrowDown)) - i32::from(input.key_down(Key::ArrowUp)),
        ];
        // An existing hold can change direction/modifiers, but interrupted input
        // must receive a fresh press before repeat events may move it again.
        if hold.is_some_and(|hold| {
            hold.direction != held_direction || hold.fast != input.modifiers.shift
        }) {
            hold = Some(Hold {
                direction: held_direction,
                fast: input.modifiers.shift,
                started: input.time,
            });
        }
        let mut steps = [0_i32; 2];
        for event in &input.events {
            if let Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = event
                && !reserved(*modifiers)
                && let Some(direction) = direction(*key)
            {
                if !repeat || hold.is_some_and(|hold| hold.fast != modifiers.shift) {
                    hold = Some(Hold {
                        direction: held_direction,
                        fast: modifiers.shift,
                        started: input.time,
                    });
                }
                let Some(hold) = hold else { continue };
                let stride = hold.stride(input.time);
                for axis in 0..2 {
                    steps[axis] = steps[axis].saturating_add(direction[axis] * stride);
                }
            }
        }
        let any_held = [
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
        ]
        .iter()
        .any(|key| input.key_down(*key));
        if !any_held {
            hold = None;
        } else {
            // Opposing keys cancel that axis without synthesizing movement.
            for axis in 0..2 {
                if held_direction[axis] == 0 {
                    steps[axis] = 0;
                }
            }
        }
        steps
    });
    ctx.data_mut(|data| match hold {
        Some(hold) => {
            data.insert_temp(state_id(id), hold);
        }
        None => {
            data.remove::<Hold>(state_id(id));
        }
    });
    if steps != [0, 0] {
        emit(Action::Move(id, steps));
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
}
