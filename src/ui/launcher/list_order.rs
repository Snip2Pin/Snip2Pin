//! List presentation order is independent of window stacking and pin storage.
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use eframe::egui::{self, Pos2, Rect};

use super::{Action, LAUNCHER, Locale, PinEntry, Text};

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
pub(super) struct DraggedPin {
    id: u64,
    /// Pointer offset from the row origin, in UI points.
    grab_offset: egui::Vec2,
}

fn handle_id(id: u64) -> egui::Id {
    egui::Id::new(("pin-list-drag", id))
}

pub(super) fn active(ctx: &egui::Context) -> Option<Arc<DraggedPin>> {
    egui::DragAndDrop::payload(ctx)
}

/// Keep the list slot, but let egui paint its source on the drag layer.
pub(super) fn row(
    ui: &mut egui::Ui,
    id: u64,
    height: f32,
    drag: Option<&DraggedPin>,
    paint: impl FnOnce(&mut egui::Ui, bool) -> Option<Action>,
) -> Option<Action> {
    let Some(drag) = drag.filter(|drag| drag.id == id) else {
        return paint(ui, false);
    };
    let size = egui::vec2(ui.available_width(), height);
    let (slot, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.input(|input| input.pointer.primary_down()) && ui.ctx().is_being_dragged(handle_id(id)) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
            ui.dnd_drag_source(handle_id(id), *drag, |ui| {
                // The floating row can leave the scroll area; the viewport still clips it.
                ui.set_clip_rect(Rect::EVERYTHING);
                // egui centers its drag source. Preserve the original grab point instead.
                ui.with_visual_transform(
                    egui::emath::TSTransform::from_translation(size * 0.5 - drag.grab_offset),
                    |ui| paint(ui, true),
                );
            });
        });
    }
    None
}

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
        .interact(rect, handle_id(id), egui::Sense::drag())
        .on_hover_text(locale.text(Text::DragToReorder));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            locale.text(Text::DragToReorder),
        )
    });
    let grab = ui
        .input(|input| input.pointer.press_origin())
        .unwrap_or(row.min)
        - row.min;
    response.dnd_set_drag_payload(DraggedPin {
        id,
        grab_offset: grab,
    });
    if response.drag_started() && ui.ctx().current_pass_index() == 0 {
        ui.ctx()
            .request_discard("paint the lifted list row on the drag-start frame");
    }
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
        .filter(|source| source.id != id)
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
                .move_to(source.id, id, after)
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
    if egui::DragAndDrop::payload::<DraggedPin>(ctx).is_some_and(|source| source.id == id) {
        egui::DragAndDrop::clear_payload(ctx);
    }
}

#[cfg(test)]
mod tests;
