use eframe::egui::{
    self, Color32, Pos2, Rect, ResizeDirection, Sense, TextureHandle, ViewportCommand,
};
use snip2pin::pin::METRICS;

use super::{icons::paint_copy_icon, style::PIN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    CopyPin(u64),
    ClosePin(u64),
    MenuPin(u64),
}

const MENU_PADDING: f32 = 8.0;
pub const MENU_SIZE: egui::Vec2 = egui::vec2(120.0, 72.0);

/// The web sandbox intercepts these requests; desktop hosts handle the gesture.
/// This view never computes or requests a new window position or size.
pub fn image_with_host(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    native_edges: bool,
    excluded: Option<Rect>,
    events: &dyn Fn(Action),
    mut host: impl FnMut(ViewportCommand),
) -> bool {
    let bounds = ui.max_rect();
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
    let edge = (!native_edges)
        .then(|| ui.input(|input| input.pointer.interact_pos()))
        .flatten()
        .filter(|point| !excluded.is_some_and(|rect| rect.contains(*point)))
        .and_then(|point| resize_direction(bounds, point));
    if response.hovered()
        && !native_edges
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
    if pressed.is_some_and(|point| {
        bounds.contains(point)
            && ui.clip_rect().contains(point)
            && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
            && !excluded.is_some_and(|rect| rect.contains(point))
    }) {
        let pressed_edge = (!native_edges)
            .then_some(pressed)
            .flatten()
            .and_then(|point| resize_direction(bounds, point));
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
    let double_clicked_image = response.double_clicked()
        && response.interact_pointer_pos().is_some_and(|point| {
            bounds.contains(point) && !excluded.is_some_and(|rect| rect.contains(point))
        });
    if double_clicked_image || close_requested(ui) {
        events(Action::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    response.contains_pointer()
}

pub fn controls(ui: &mut egui::Ui, id: u64, events: &dyn Fn(Action)) {
    let bounds = ui.max_rect();
    let first = Rect::from_min_size(bounds.min, egui::Vec2::splat(METRICS.button));
    let step = METRICS.button + METRICS.button_gap;
    let (copy_rect, close_rect) = if bounds.height() > bounds.width() {
        (first.translate(egui::vec2(0.0, step)), first)
    } else {
        (first, first.translate(egui::vec2(step, 0.0)))
    };
    let copy = ui.put(copy_rect, egui::Button::new("").fill(PIN.button_fill));
    copy.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "复制")
    });
    paint_copy_icon(ui, copy.rect);
    if copy.clicked() {
        events(Action::CopyPin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    let close = ui.put(
        close_rect,
        egui::Button::new(egui::RichText::new("×").color(Color32::WHITE)).fill(PIN.button_fill),
    );
    close.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "关闭")
    });
    if close.clicked() {
        events(Action::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
}

pub fn menu(ui: &mut egui::Ui, id: u64, events: &dyn Fn(Action)) {
    let bounds = ui.max_rect().shrink(MENU_PADDING);
    let height = (bounds.height() - METRICS.button_gap) / 2.0;
    for (index, (label, action)) in [
        ("复制", Action::CopyPin(id)),
        ("关闭", Action::ClosePin(id)),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(
            bounds.min + egui::vec2(0.0, index as f32 * (height + METRICS.button_gap)),
            egui::vec2(bounds.width(), height),
        );
        if ui.put(rect, egui::Button::new(label)).clicked() {
            events(action);
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

fn close_requested(ui: &egui::Ui) -> bool {
    ui.input(|input| input.viewport().close_requested() || input.key_pressed(egui::Key::Escape))
}

fn resize_direction(bounds: Rect, point: Pos2) -> Option<ResizeDirection> {
    if !bounds.contains(point) {
        return None;
    }
    let hit = egui::vec2(
        METRICS.inset.min(bounds.width() / 3.0),
        METRICS.inset.min(bounds.height() / 3.0),
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
