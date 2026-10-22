//! List presentation order is independent of window stacking and pin storage.
use std::collections::{HashMap, HashSet};

use eframe::egui::{self, Pos2, Rect};

use super::{LAUNCHER, Locale, PinEntry, Text};

#[derive(Clone, Default)]
struct Order(Vec<u64>);

impl Order {
    fn indices(&mut self, ids: &[u64]) -> Vec<usize> {
        let positions: HashMap<_, _> = ids
            .iter()
            .enumerate()
            .map(|(index, &id)| (id, index))
            .collect();
        self.0.retain(|id| positions.contains_key(id));
        let known: HashSet<_> = self.0.iter().copied().collect();
        // New captures precede existing rows without resetting their manual order.
        self.0
            .splice(0..0, ids.iter().copied().filter(|id| !known.contains(id)));
        self.0
            .iter()
            .filter_map(|id| positions.get(id).copied())
            .collect()
    }

    fn move_to(&mut self, source: u64, target: u64, after: bool) -> bool {
        if source == target || !self.0.contains(&source) || !self.0.contains(&target) {
            return false;
        }
        self.0.retain(|&id| id != source);
        if let Some(index) = self.0.iter().position(|&id| id == target) {
            self.0.insert(index + usize::from(after), source);
        }
        true
    }
}

#[derive(Clone, Copy)]
struct DraggedPin(u64);

fn state_id() -> egui::Id {
    egui::Id::new("launcher-pin-list-order")
}

pub(super) fn indices(ctx: &egui::Context, pins: &[PinEntry<'_>]) -> Vec<usize> {
    if !ctx.input(|input| input.focused)
        && egui::DragAndDrop::has_payload_of_type::<DraggedPin>(ctx)
    {
        egui::DragAndDrop::clear_payload(ctx);
    }
    let ids = pins.iter().map(|pin| pin.id).collect::<Vec<_>>();
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Order>(state_id())
            .indices(&ids)
    })
}

pub(super) fn handle(ui: &mut egui::Ui, row: Rect, id: u64, locale: Locale) -> Rect {
    let content = row.shrink2(egui::Vec2::from(LAUNCHER.pin_row_padding));
    let rect = Rect::from_center_size(
        egui::pos2((row.left() + content.left()) / 2.0, row.center().y),
        egui::vec2(LAUNCHER.pin_row_padding[0], row.height()),
    );
    let response = ui
        .interact(rect, ui.id().with("reorder"), egui::Sense::drag())
        .on_hover_text(locale.text(Text::DragToReorder));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            locale.text(Text::DragToReorder),
        )
    });
    response.dnd_set_drag_payload(DraggedPin(id));
    let color = if response.hovered() || response.dragged() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let spacing = rect.width() / 3.0;
    for x in [-0.5, 0.5] {
        for y in [-1.0, 0.0, 1.0] {
            ui.painter().circle_filled(
                rect.center() + egui::vec2(x, y) * spacing,
                spacing / 4.0,
                color,
            );
        }
    }
    rect
}

pub(super) fn drop_target(ui: &egui::Ui, response: &egui::Response, id: u64) {
    if !ui.is_enabled() {
        return;
    }
    let Some(source) = response
        .dnd_hover_payload::<DraggedPin>()
        .filter(|source| source.0 != id)
    else {
        return;
    };
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let after = pointer.y > response.rect.center().y;
    let y = if after {
        response.rect.bottom()
    } else {
        response.rect.top()
    };
    ui.painter().line_segment(
        [
            Pos2::new(response.rect.left(), y),
            Pos2::new(response.rect.right(), y),
        ],
        egui::Stroke::new(
            ui.ctx().pixels_per_point().recip(),
            ui.visuals().selection.stroke.color,
        ),
    );
    if response.dnd_release_payload::<DraggedPin>().is_some() {
        ui.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<Order>(state_id())
                .move_to(source.0, id, after)
        });
        ui.ctx().request_repaint();
    }
}

pub(super) fn scroll(ui: &egui::Ui, row_height: f32) {
    if ui.is_enabled()
        && egui::DragAndDrop::has_payload_of_type::<DraggedPin>(ui.ctx())
        && let Some(pointer) = ui
            .ctx()
            .pointer_interact_pos()
            .filter(|point| ui.clip_rect().contains(*point))
    {
        // Let ScrollArea reveal the destination when the drag reaches an edge.
        ui.scroll_to_rect(
            Rect::from_center_size(pointer, egui::Vec2::splat(row_height)),
            None,
        );
    }
}

pub(super) fn forget(ctx: &egui::Context, id: u64) {
    ctx.data_mut(|data| {
        let order = data.get_temp_mut_or_default::<Order>(state_id());
        order.0.retain(|&existing| existing != id);
        if order.0.is_empty() {
            data.remove::<Order>(state_id());
        }
    });
    if egui::DragAndDrop::payload::<DraggedPin>(ctx).is_some_and(|source| source.0 == id) {
        egui::DragAndDrop::clear_payload(ctx);
    }
}

#[cfg(test)]
mod tests;
