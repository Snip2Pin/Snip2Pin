//! Time-based held-arrow input; native windows remain authoritative for geometry.
use eframe::egui::{self, Event, Key, Vec2};
use snip2pin::pin::FAST_MOVE_STEPS;

use super::{Action, input::reserved};

struct Pace {
    initial: f64,
    acceleration: f64,
    maximum: f64,
}

// Speeds use the same native steps as a fine arrow-key tap, per second.
const NORMAL: Pace = Pace {
    initial: 60.0,
    acceleration: 1200.0,
    maximum: 900.0,
};
const FAST: Pace = Pace {
    initial: 120.0,
    acceleration: 2400.0,
    maximum: 1800.0,
};
// Discard catch-up after a stalled frame, rather than teleporting the window.
const MAX_FRAME_GAP: f64 = 0.1;

impl Pace {
    fn distance(&self, seconds: f64) -> f64 {
        let accelerating = seconds.min((self.maximum - self.initial) / self.acceleration);
        self.initial * accelerating
            + 0.5 * self.acceleration * accelerating.powi(2)
            + self.maximum * (seconds - accelerating)
    }
}

#[derive(Clone, Copy)]
struct Hold {
    direction: Vec2,
    time: f64,
    age: f64,
    fraction: Vec2,
}

fn state_id(id: u64) -> egui::Id {
    egui::Id::new(("pin-held-arrows", id))
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
    let (steps, active) = ui.input(|input| {
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
            return ([0, 0], false);
        }
        let mut steps = [0_i32; 2];
        let mut fresh = false;
        for event in &input.events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
                && !reserved(*modifiers)
                && let Some(direction) = direction(*key)
            {
                fresh = true;
                let stride = if modifiers.shift { FAST_MOVE_STEPS } else { 1 };
                for axis in 0..2 {
                    steps[axis] = steps[axis].saturating_add(direction[axis] * stride);
                }
            }
        }
        let keys = [
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
        ];
        if !keys.iter().any(|key| input.key_down(*key)) {
            hold = None;
            return (steps, false);
        }
        let direction = egui::vec2(
            f32::from(input.key_down(Key::ArrowRight)) - f32::from(input.key_down(Key::ArrowLeft)),
            f32::from(input.key_down(Key::ArrowDown)) - f32::from(input.key_down(Key::ArrowUp)),
        )
        .normalized();
        // Focus/lock transitions must not resume a still-held or repeated key.
        if fresh || hold.is_some_and(|hold| hold.direction != direction) {
            hold = Some(Hold {
                direction,
                time: input.time,
                age: 0.0,
                fraction: Vec2::ZERO,
            });
        } else if let Some(hold) = &mut hold {
            let elapsed = (input.time - hold.time).clamp(0.0, MAX_FRAME_GAP);
            hold.time = input.time;
            let pace = if input.modifiers.shift {
                &FAST
            } else {
                &NORMAL
            };
            let distance = pace.distance(hold.age + elapsed) - pace.distance(hold.age);
            hold.age += elapsed;
            hold.fraction += direction * distance as f32;
            for (axis, step) in steps.iter_mut().enumerate() {
                let whole = hold.fraction[axis].trunc() as i32;
                hold.fraction[axis] -= whole as f32;
                *step = step.saturating_add(whole);
            }
        }
        // Opposing keys stop that axis immediately, including a fresh press.
        for axis in 0..2 {
            if direction[axis] == 0.0 {
                steps[axis] = 0;
            }
        }
        (steps, hold.is_some() && direction != Vec2::ZERO)
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
    if active {
        ctx.request_repaint();
    }
}
