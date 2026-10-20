use eframe::egui::{
    self, Color32, Pos2, Rect, ResizeDirection, Sense, TextureHandle, ViewportCommand,
};
use snip2pin::pin::{Control, METRICS, Toolbar, Zoom};

use super::{
    icons::{paint_copy_icon, paint_lock_icon},
    style::PIN,
};

mod input;
mod keyboard;
mod session;

pub fn forget_input(ctx: &egui::Context, id: u64) {
    keyboard::forget(ctx, id);
    ctx.data_mut(|data| {
        data.remove::<session::Session>(session::key(id));
        if data
            .get_temp::<session::Owner>(session::owner_key())
            .is_some_and(|owner| owner.pin == id)
        {
            data.remove::<session::Owner>(session::owner_key());
        }
    });
}

/// Route a continuing gesture delivered to another window of this app.
/// Input delivered to other processes remains under the operating system.
pub fn route_gesture(ui: &egui::Ui, emit: &dyn Fn(Action)) {
    let owner = ui
        .ctx()
        .data(|data| data.get_temp::<session::Owner>(session::owner_key()));
    if let Some(owner) = owner
        && owner.viewport != ui.ctx().viewport_id()
    {
        input::adjust(ui, owner.pin, false, Interaction::default(), true, emit);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    UnlockAll,
    UnlockHint(u64),
    CopyPin(u64),
    ClosePin(u64),
    MenuPin(u64),
    ToggleLock(u64),
    Zoom(u64, Vec<Zoom>),
    Move(u64, [i32; 2]),
}

const MENU_PADDING: f32 = 8.0;
const MENU_ROWS: usize = 4;
pub const MENU_SIZE: egui::Vec2 = egui::vec2(
    PIN.menu_width,
    MENU_PADDING * 2.0
        + METRICS.button * MENU_ROWS as f32
        + METRICS.button_gap * (MENU_ROWS - 1) as f32,
);

#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction {
    pub native_edges: bool,
    pub locked: bool,
    pub excluded: Option<Rect>,
    /// Virtual hosts supply per-image focus; native hosts use viewport focus.
    pub keyboard_focus: Option<bool>,
    /// Native hosts can supply their authoritative double-click press.
    pub native_double_click: Option<bool>,
}

/// The web sandbox intercepts these requests; desktop hosts handle the gesture.
/// This view never computes or requests a new window position or size.
pub fn image_with_host(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    interaction: Interaction,
    events: &dyn Fn(Action),
    mut host: impl FnMut(ViewportCommand),
) -> bool {
    let Interaction {
        native_edges,
        locked,
        excluded,
        keyboard_focus,
        native_double_click,
    } = interaction;
    let bounds = ui.max_rect();
    let focused =
        keyboard_focus.unwrap_or_else(|| ui.input(|input| input.viewport().focused == Some(true)));
    input::adjust(ui, id, focused, interaction, false, events);
    let resize_inset = if focused {
        PIN.focused_resize_inset
    } else {
        METRICS.inset
    };
    let response = ui.interact(
        bounds,
        ui.id().with(("pin-image", id)),
        Sense::click_and_drag(),
    );
    if ui.ctx().current_pass_index() == 0
        && ui.input(|input| input.pointer.secondary_clicked())
        && ui
            .input(|input| input.pointer.interact_pos())
            .is_some_and(|point| {
                bounds.contains(point)
                    && ui.clip_rect().contains(point)
                    && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
            })
    {
        events(Action::MenuPin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    let edge = (!native_edges && !locked)
        .then(|| ui.input(|input| input.pointer.interact_pos()))
        .flatten()
        .filter(|point| !excluded.is_some_and(|rect| rect.contains(*point)))
        .and_then(|point| resize_direction(bounds, point, resize_inset));
    if response.hovered()
        && !native_edges
        && !locked
        && !ui
            .input(|input| input.pointer.interact_pos())
            .is_some_and(|point| excluded.is_some_and(|rect| rect.contains(point)))
    {
        if let Some(direction) = edge {
            ui.ctx().set_cursor_icon(resize_cursor(direction));
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    // Native drag/resize APIs need the initiating button press, before an egui
    // drag threshold turns it into a later mouse-move event.
    let pressed = ui.input(|input| {
        input
            .pointer
            .primary_pressed()
            .then(|| input.pointer.press_origin())
            .flatten()
    });
    let pressed_image = pressed.is_some_and(|point| {
        bounds.contains(point)
            && ui.clip_rect().contains(point)
            && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
            && !excluded.is_some_and(|rect| rect.contains(point))
    });
    if !locked && pressed_image && native_double_click != Some(true) {
        let pressed_edge = (!native_edges)
            .then_some(pressed)
            .flatten()
            .and_then(|point| resize_direction(bounds, point, resize_inset));
        match pressed_edge {
            Some(direction) => host(ViewportCommand::BeginResize(direction)),
            None => host(ViewportCommand::StartDrag),
        }
    }
    ui.painter().image(
        texture.id(),
        bounds,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    if locked
        && ui.ctx().current_pass_index() == 0
        && pressed.is_some_and(|point| {
            bounds.contains(point)
                && ui.clip_rect().contains(point)
                && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
                && !excluded.is_some_and(|rect| rect.contains(point))
        })
    {
        events(Action::UnlockHint(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    let double_clicked_image = native_double_click.map_or_else(
        || {
            response.double_clicked()
                && response.interact_pointer_pos().is_some_and(|point| {
                    bounds.contains(point) && !excluded.is_some_and(|rect| rect.contains(point))
                })
        },
        |double| {
            double
                && ui
                    .input(|input| {
                        input.events.iter().rev().find_map(|event| match event {
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                ..
                            } => Some(*pos),
                            _ => None,
                        })
                    })
                    .is_some_and(|point| {
                        bounds.contains(point)
                            && ui.clip_rect().contains(point)
                            && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
                            && !excluded.is_some_and(|rect| rect.contains(point))
                    })
        },
    );
    if ui.ctx().current_pass_index() == 0
        && !locked
        && (double_clicked_image || close_requested(ui))
    {
        events(Action::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if locked && ui.input(|input| input.viewport().close_requested()) {
        host(ViewportCommand::CancelClose);
    }
    response.contains_pointer()
}

pub fn controls(
    ui: &mut egui::Ui,
    id: u64,
    locked: bool,
    toolbar: Toolbar,
    events: &dyn Fn(Action),
) {
    let locale = crate::i18n::locale(ui.ctx());
    let step = METRICS.button + METRICS.button_gap;
    for (index, control) in Control::ALL
        .into_iter()
        .skip(Control::ALL.len().saturating_sub(toolbar.count))
        .enumerate()
    {
        let (label, action) = match control {
            Control::Copy => (crate::i18n::Text::Copy, Action::CopyPin(id)),
            Control::Lock => (
                if locked {
                    crate::i18n::Text::Unlock
                } else {
                    crate::i18n::Text::Lock
                },
                Action::ToggleLock(id),
            ),
            Control::Close => (crate::i18n::Text::Close, Action::ClosePin(id)),
        };
        let rect = Rect::from_min_size(
            toolbar.bounds.min + egui::vec2(index as f32 * step, 0.0),
            egui::Vec2::splat(METRICS.button),
        );
        let enabled = control != Control::Close || !locked;
        let response = ui
            .push_id(("pin-control", id, control), |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.put(
                        rect,
                        egui::Button::new(
                            egui::RichText::new(if control == Control::Close { "×" } else { "" })
                                .color(if enabled {
                                    Color32::WHITE
                                } else {
                                    Color32::GRAY
                                }),
                        )
                        .fill(PIN.button_fill),
                    )
                })
                .inner
            })
            .inner;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled() && enabled,
                locale.text(label),
            )
        });
        match control {
            Control::Copy => paint_copy_icon(ui, response.rect),
            Control::Lock => paint_lock_icon(ui, response.rect, locked, Color32::WHITE),
            Control::Close => {}
        }
        if response.clicked() {
            events(action);
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

pub fn menu(ui: &mut egui::Ui, id: u64, locked: bool, events: &dyn Fn(Action)) {
    let locale = crate::i18n::locale(ui.ctx());
    let bounds = ui.max_rect().shrink(MENU_PADDING);
    let height = (bounds.height() - METRICS.button_gap * (MENU_ROWS - 1) as f32) / MENU_ROWS as f32;
    for (index, (label, action, enabled)) in [
        (
            locale.text(crate::i18n::Text::Copy),
            Action::CopyPin(id),
            true,
        ),
        (
            locale.text(if locked {
                crate::i18n::Text::Unlock
            } else {
                crate::i18n::Text::Lock
            }),
            Action::ToggleLock(id),
            true,
        ),
        (
            locale.text(crate::i18n::Text::RestoreSize),
            Action::Zoom(id, vec![Zoom::Original]),
            !locked,
        ),
        (
            locale.text(crate::i18n::Text::Close),
            Action::ClosePin(id),
            !locked,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(
            bounds.min + egui::vec2(0.0, index as f32 * (height + METRICS.button_gap)),
            egui::vec2(bounds.width(), height),
        );
        if ui
            .add_enabled_ui(enabled, |ui| ui.put(rect, egui::Button::new(label)))
            .inner
            .clicked()
        {
            events(action);
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

fn close_requested(ui: &egui::Ui) -> bool {
    ui.input(|input| input.viewport().close_requested() || input.key_pressed(egui::Key::Escape))
}

fn resize_direction(bounds: Rect, point: Pos2, inset: f32) -> Option<ResizeDirection> {
    if !bounds.contains(point) {
        return None;
    }
    let hit = egui::vec2(
        inset.min(bounds.width() / 3.0),
        inset.min(bounds.height() / 3.0),
    );
    let horizontal = if point.x < bounds.min.x + hit.x {
        -1
    } else if point.x > bounds.max.x - hit.x {
        1
    } else {
        0
    };
    let vertical = if point.y < bounds.min.y + hit.y {
        -1
    } else if point.y > bounds.max.y - hit.y {
        1
    } else {
        0
    };
    use ResizeDirection::*;
    match (horizontal, vertical) {
        (-1, -1) => Some(NorthWest),
        (0, -1) => Some(North),
        (1, -1) => Some(NorthEast),
        (-1, 0) => Some(West),
        (1, 0) => Some(East),
        (-1, 1) => Some(SouthWest),
        (0, 1) => Some(South),
        (1, 1) => Some(SouthEast),
        _ => None,
    }
}

fn resize_cursor(direction: ResizeDirection) -> egui::CursorIcon {
    use ResizeDirection::*;
    match direction {
        North | South => egui::CursorIcon::ResizeVertical,
        West | East => egui::CursorIcon::ResizeHorizontal,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
    }
}

#[cfg(test)]
mod tests;
