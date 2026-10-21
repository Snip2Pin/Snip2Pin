//! Translate explicit user input into semantic adjustments, never geometry.
use eframe::egui::{self, Event, Modifiers, MouseWheelUnit};
use snip2pin::pin::{FAST_ZOOM_FACTOR, FINE_ZOOM_FACTOR, Zoom, ZoomAnchor};

use super::{
    Action, Interaction,
    session::{Owner, Session, Target},
};

pub(super) fn reserved(modifiers: Modifiers) -> bool {
    modifiers.ctrl || modifiers.command || modifiers.mac_cmd || modifiers.alt
}

fn flush_scroll(
    steps: &mut [f32; 2],
    pinch: &mut f32,
    anchor: &mut Option<ZoomAnchor>,
    zooms: &mut Vec<Zoom>,
) {
    let factor = FINE_ZOOM_FACTOR.powf(steps[0]) * FAST_ZOOM_FACTOR.powf(steps[1]) * *pinch;
    *steps = [0.0; 2];
    *pinch = 1.0;
    let anchor = anchor.take();
    if factor.is_finite() && factor > 0.0 && factor != 1.0 {
        zooms.push(match anchor {
            Some(anchor) => Zoom::At { factor, anchor },
            None => Zoom::Scale(factor),
        });
    }
}

pub(super) fn adjust(
    ui: &egui::Ui,
    id: u64,
    focused: bool,
    interaction: Interaction,
    continuation: bool,
    emit: &dyn Fn(Action),
) {
    if !continuation
        && ui.ctx().current_pass_index() == 0
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
    if interaction.locked
        || ui.ctx().text_edit_focused()
        || egui::Popup::is_any_open(ui.ctx())
        || ui.ctx().current_pass_index() != 0
    {
        if ui.ctx().current_pass_index() == 0 {
            super::forget_input(ui.ctx(), id);
        }
        return;
    }
    let bounds = ui.max_rect();
    let owner = ui
        .ctx()
        .data(|data| data.get_temp::<Owner>(super::session::owner_key()));
    let page_size = if continuation {
        ui.input(|input| {
            input
                .raw
                .viewports
                .get(&owner?.viewport)?
                .inner_rect
                .map(|rect| rect.size())
        })
        .unwrap_or(egui::Vec2::ZERO)
    } else {
        bounds.size()
    };
    let stamp = (ui.ctx().viewport_id(), ui.input(|input| input.time));
    let gesture_enabled = owner.is_none_or(|owner| owner.pin == id && owner.handled != stamp);
    let pointer_down = ui.input(|input| input.pointer.any_down());
    let hovered = ui.input(|input| input.pointer.hover_pos()).filter(|point| {
        bounds.contains(*point)
            && ui.clip_rect().contains(*point)
            && !interaction
                .toolbar
                .is_some_and(|toolbar| toolbar.bounds.contains(*point))
            && ui.ctx().layer_id_at(*point) == Some(ui.layer_id())
    });
    let mut session = ui
        .ctx()
        .data(|data| data.get_temp::<Session>(super::session::key(id)));
    if pointer_down
        || ui.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, Event::WindowFocused(false)))
        })
    {
        session = None;
    }
    let points_per_line = ui
        .ctx()
        .options(|options| options.input_options.line_scroll_speed);
    let zoom = ui.ctx().zoom_factor();
    let target = Target {
        hovered,
        bounds,
        space: interaction.space,
    };
    let scroll_zoom_speed = ui
        .ctx()
        .options(|options| options.input_options.scroll_zoom_speed);
    let zooms = ui.input(|input| {
        let mut zooms = Vec::new();
        let mut scroll = [0.0; 2];
        let mut pinch = 1.0;
        let mut anchor = None;
        let explicit_zoom = input
            .events
            .iter()
            .any(|event| matches!(event, Event::Zoom(_)));
        for event in &input.events {
            if gesture_enabled
                && matches!(
                    event,
                    Event::MouseWheel {
                        phase: egui::TouchPhase::End | egui::TouchPhase::Cancel,
                        ..
                    }
                )
            {
                session = None;
                continue;
            }
            match event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if focused && !reserved(*modifiers) => {
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
                        _ => None,
                    };
                    if let Some(command) = command {
                        flush_scroll(&mut scroll, &mut pinch, &mut anchor, &mut zooms);
                        zooms.push(command);
                    }
                }
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    phase,
                } if gesture_enabled
                    && !pointer_down
                    && (!reserved(*modifiers)
                        || (modifiers.ctrl && !modifiers.alt && !modifiers.mac_cmd)) =>
                {
                    // Plain precision scrolling does not manipulate windows.
                    // Ctrl+wheel remains a pinch fallback; native Zoom wins.
                    if (explicit_zoom && modifiers.ctrl)
                        || (*unit == MouseWheelUnit::Point && (!modifiers.ctrl || explicit_zoom))
                    {
                        continue;
                    }
                    let Some(target) = Session::target(
                        &mut session,
                        Some(*phase),
                        input.time,
                        Target {
                            hovered: if continuation { None } else { target.hovered },
                            ..target
                        },
                    ) else {
                        continue;
                    };
                    if anchor.is_some() && anchor != Some(target) {
                        flush_scroll(&mut scroll, &mut pinch, &mut anchor, &mut zooms);
                    }
                    anchor = Some(target);
                    if modifiers.ctrl {
                        // Precision touchpads and browsers can synthesize pinch
                        // as Ctrl+wheel. Prefer a native zoom event if supplied.
                        if !explicit_zoom {
                            let points = match unit {
                                MouseWheelUnit::Line => *delta * points_per_line,
                                MouseWheelUnit::Point => *delta * zoom,
                                MouseWheelUnit::Page => *delta * page_size * zoom,
                            };
                            let factor = ((points.x + points.y) * scroll_zoom_speed).exp();
                            if factor.is_finite() && factor > 0.0 {
                                pinch *= factor;
                            }
                        }
                        continue;
                    }
                    let steps = match unit {
                        MouseWheelUnit::Line => delta.y,
                        MouseWheelUnit::Page
                            if points_per_line.is_finite() && points_per_line > 0.0 =>
                        {
                            delta.y * page_size.y * zoom / points_per_line
                        }
                        _ => 0.0,
                    };
                    if steps.is_finite() {
                        scroll[usize::from(modifiers.shift)] += steps;
                    }
                }
                Event::Zoom(factor)
                    if gesture_enabled
                        && !pointer_down
                        && !input.modifiers.alt
                        && !input.modifiers.mac_cmd
                        && (!input.modifiers.command || input.modifiers.ctrl)
                        && factor.is_finite()
                        && *factor > 0.0 =>
                {
                    let Some(target) = Session::target(
                        &mut session,
                        None,
                        input.time,
                        Target {
                            hovered: if continuation { None } else { target.hovered },
                            ..target
                        },
                    ) else {
                        continue;
                    };
                    if anchor.is_some() && anchor != Some(target) {
                        flush_scroll(&mut scroll, &mut pinch, &mut anchor, &mut zooms);
                    }
                    anchor = Some(target);
                    pinch *= factor;
                }
                _ => {}
            }
        }
        flush_scroll(&mut scroll, &mut pinch, &mut anchor, &mut zooms);
        zooms
    });
    if gesture_enabled {
        ui.ctx().data_mut(|data| match session {
            Some(session) => {
                data.insert_temp(super::session::key(id), session);
                if session.owns_target() {
                    data.insert_temp(
                        super::session::owner_key(),
                        Owner {
                            pin: id,
                            viewport: owner.map_or(stamp.0, |owner| owner.viewport),
                            handled: stamp,
                        },
                    );
                } else if owner.is_some_and(|owner| owner.pin == id) {
                    data.remove::<Owner>(super::session::owner_key());
                }
            }
            None => {
                data.remove_by_type::<Session>();
                data.remove::<Owner>(super::session::owner_key());
            }
        });
    }
    if !zooms.is_empty() {
        emit(Action::Zoom(id, zooms));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if !continuation {
        super::keyboard::adjust(ui, id, focused, emit);
    }
}
