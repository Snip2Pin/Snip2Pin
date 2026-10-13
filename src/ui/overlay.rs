use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use snip2pin::geometry::{self, PixelRect};

pub struct Selection {
    pub texture: TextureHandle,
    pub pixels: [u32; 2],
    pub start: Option<Pos2>,
    pub focus_requested: bool,
}

pub enum Action {
    Cancel,
    Select(PixelRect),
}

pub fn show(ui: &mut egui::Ui, selection: &mut Selection) -> Option<Action> {
    if !selection.focus_requested {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        selection.focus_requested = true;
    }
    let bounds = ui.max_rect();
    ui.painter().image(
        selection.texture.id(),
        bounds,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    let response = ui.allocate_rect(bounds, Sense::drag());
    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    if ui.input(|input| input.key_pressed(egui::Key::Escape) || input.viewport().close_requested())
    {
        return Some(Action::Cancel);
    }
    if response.drag_started() {
        selection.start = ui.input(|input| input.pointer.press_origin());
    }
    let current = ui.input(|input| input.pointer.interact_pos());
    if let (Some(start), Some(current)) = (selection.start, current) {
        let rect = Rect::from_two_pos(start, current).intersect(bounds);
        ui.painter().rect_stroke(
            rect,
            0,
            Stroke::new(2.0, Color32::WHITE),
            StrokeKind::Inside,
        );
        if response.drag_stopped() {
            selection.start = None;
            let result = geometry::selection(
                [
                    f64::from(start.x - bounds.min.x),
                    f64::from(start.y - bounds.min.y),
                ],
                [
                    f64::from(current.x - bounds.min.x),
                    f64::from(current.y - bounds.min.y),
                ],
                [f64::from(bounds.width()), f64::from(bounds.height())],
                selection.pixels,
            );
            return result.map(Action::Select);
        }
    }
    let hint = Rect::from_center_size(
        Pos2::new(bounds.center().x, bounds.min.y + 38.0),
        egui::vec2(340.0, 36.0),
    );
    if selection.start.is_none() {
        ui.painter()
            .rect_filled(hint, 8, Color32::from_black_alpha(180));
        ui.painter().text(
            hint.center(),
            egui::Align2::CENTER_CENTER,
            "拖动框选区域，松手置顶 · Esc 取消",
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
    }
    None
}
