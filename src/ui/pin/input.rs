//! Translate explicit user input into semantic adjustments, never geometry.
use eframe::egui::{self, Event, Modifiers, MouseWheelUnit, Rect};
use snip2pin::pin::{FAST_MOVE_STEPS, FAST_ZOOM_FACTOR, FINE_ZOOM_FACTOR, Zoom};

use super::Action;

fn reserved(modifiers: Modifiers) -> bool {
    modifiers.ctrl || modifiers.command || modifiers.mac_cmd || modifiers.alt
}

fn flush_scroll(steps: &mut [f32; 2], pinch: &mut f32, zooms: &mut Vec<Zoom>) {
    let factor = FINE_ZOOM_FACTOR.powf(steps[0]) * FAST_ZOOM_FACTOR.powf(steps[1]) * *pinch;
    *steps = [0.0; 2];
    *pinch = 1.0;
    if factor.is_finite() && factor > 0.0 && factor != 1.0 {
        zooms.push(Zoom::Scale(factor));
    }
}

pub(super) fn adjust(
    ui: &egui::Ui,
    id: u64,
    focused: bool,
    locked: bool,
    excluded: Option<Rect>,
    emit: &dyn Fn(Action),
) {
    if ui.ctx().current_pass_index() == 0
        && ui.input(|input| {
            input
                .events
                .iter()
                .any(|event| snip2pin::settings::Binding::unlock_all().matches(event))
        })
    {
        emit(Action::UnlockAll);
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if locked || ui.ctx().text_edit_focused() || ui.ctx().current_pass_index() != 0 {
        return;
    }
    let bounds = ui.max_rect();
    let wheel_target = ui.input(|input| !input.pointer.any_down())
        && ui
            .input(|input| input.pointer.hover_pos())
            .is_some_and(|point| {
                bounds.contains(point)
                    && ui.clip_rect().contains(point)
                    && !excluded.is_some_and(|rect| rect.contains(point))
                    && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
            });
    let points_per_line = ui
        .ctx()
        .options(|options| options.input_options.line_scroll_speed);
    let zoom = ui.ctx().zoom_factor();
    let scroll_zoom_speed = ui
        .ctx()
        .options(|options| options.input_options.scroll_zoom_speed);
    let (zooms, movement, pan) = ui.input(|input| {
        let mut zooms = Vec::new();
        let mut movement = [0_i32; 2];
        let mut scroll = [0.0; 2];
        let mut pinch = 1.0;
        let mut pan = egui::Vec2::ZERO;
        let explicit_zoom = input
            .events
            .iter()
            .any(|event| matches!(event, Event::Zoom(_)));
        for event in &input.events {
            match event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if focused && !reserved(*modifiers) => {
                    let step = if modifiers.shift { FAST_MOVE_STEPS } else { 1 };
                    let command = match key {
                        egui::Key::Plus | egui::Key::Equals => Some(if modifiers.shift {
                            Zoom::Scale(FAST_ZOOM_FACTOR)
                        } else {
                            Zoom::In
                        }),
                        egui::Key::Minus => Some(if modifiers.shift {
                            Zoom::Scale(FAST_ZOOM_FACTOR.recip())
                        } else {
                            Zoom::Out
                        }),
                        egui::Key::Num0 => Some(Zoom::Original),
                        egui::Key::ArrowLeft => {
                            movement[0] = movement[0].saturating_sub(step);
                            None
                        }
                        egui::Key::ArrowRight => {
                            movement[0] = movement[0].saturating_add(step);
                            None
                        }
                        egui::Key::ArrowUp => {
                            movement[1] = movement[1].saturating_sub(step);
                            None
                        }
                        egui::Key::ArrowDown => {
                            movement[1] = movement[1].saturating_add(step);
                            None
                        }
                        _ => None,
                    };
                    if let Some(command) = command {
                        flush_scroll(&mut scroll, &mut pinch, &mut zooms);
                        zooms.push(command);
                    }
                }
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    phase,
                } if wheel_target
                    && (!reserved(*modifiers)
                        || (modifiers.ctrl && !modifiers.alt && !modifiers.mac_cmd))
                    && *phase != egui::TouchPhase::Cancel =>
                {
                    if modifiers.ctrl {
                        // Precision touchpads and browsers can synthesize pinch
                        // as Ctrl+wheel. Prefer a native zoom event if supplied.
                        if !explicit_zoom {
                            let points = match unit {
                                MouseWheelUnit::Line => *delta * points_per_line,
                                MouseWheelUnit::Point => *delta * zoom,
                                MouseWheelUnit::Page => *delta * bounds.size() * zoom,
                            };
                            let factor = ((points.x + points.y) * scroll_zoom_speed).exp();
                            if factor.is_finite() && factor > 0.0 {
                                pinch *= factor;
                            }
                        }
                        continue;
                    }
                    if *unit == MouseWheelUnit::Point {
                        if delta.is_finite() && !explicit_zoom {
                            pan += *delta;
                        }
                        continue;
                    }
                    let steps = match unit {
                        MouseWheelUnit::Line => delta.y,
                        MouseWheelUnit::Page
                            if points_per_line.is_finite() && points_per_line > 0.0 =>
                        {
                            delta.y * bounds.height() * zoom / points_per_line
                        }
                        _ => 0.0,
                    };
                    if steps.is_finite() {
                        scroll[usize::from(modifiers.shift)] += steps;
                    }
                }
                Event::Zoom(factor)
                    if wheel_target
                        && !input.modifiers.alt
                        && !input.modifiers.mac_cmd
                        && (!input.modifiers.command || input.modifiers.ctrl)
                        && factor.is_finite()
                        && *factor > 0.0 =>
                {
                    pinch *= factor;
                }
                _ => {}
            }
        }
        flush_scroll(&mut scroll, &mut pinch, &mut zooms);
        (zooms, movement, pan)
    });
    if !zooms.is_empty() {
        emit(Action::Zoom(id, zooms));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if movement != [0, 0] {
        emit(Action::Move(id, movement));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if pan.is_finite() && pan != egui::Vec2::ZERO {
        emit(Action::Pan(id, pan));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
}
