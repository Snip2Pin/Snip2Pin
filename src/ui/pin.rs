use std::sync::mpsc::Sender;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, ViewportCommand};

use crate::app::Event;

pub fn show(ui: &mut egui::Ui, id: u64, texture: &TextureHandle, events: &Sender<Event>) {
    let bounds = ui.max_rect();
    ui.painter().image(
        texture.id(),
        bounds,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    let close_rect = Rect::from_min_size(
        Pos2::new((bounds.max.x - 30.0).max(bounds.min.x), bounds.min.y + 4.0),
        egui::vec2(26.0, 26.0),
    );
    let response = ui.interact(bounds, ui.id().with("image-drag"), Sense::click_and_drag());
    let on_close = ui
        .input(|input| input.pointer.interact_pos())
        .is_some_and(|point| close_rect.contains(point));
    if response.drag_started() && !on_close {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    let close = ui.put(
        close_rect,
        egui::Button::new("×").fill(Color32::from_black_alpha(160)),
    );
    if close.clicked()
        || response.double_clicked()
        || ui.input(|input| {
            input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
        })
    {
        let _ = events.send(Event::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
}
