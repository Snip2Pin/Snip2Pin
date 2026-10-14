use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use snip2pin::geometry::{self, PixelRect};

use super::style::OVERLAY;

pub struct Selection {
    pub texture: TextureHandle,
    pub pixels: [u32; 2],
    pub start: Option<Pos2>,
    pub focus_requested: bool,
}

pub enum Action {
    Activate,
    Cancel,
    Select(PixelRect),
}

pub fn show(ui: &mut egui::Ui, selection: &mut Selection, active: bool) -> Option<Action> {
    let bounds = ui.max_rect();
    let activate = !active
        && ui.input(|input| {
            input.events.iter().any(|event| match event {
                egui::Event::PointerMoved(point) => bounds.contains(*point),
                _ => false,
            })
        });
    if active && !selection.focus_requested {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        selection.focus_requested = true;
    }
    if !active {
        selection.focus_requested = false;
    }
    ui.painter().image(
        selection.texture.id(),
        bounds,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    if active {
        ui.painter().rect_stroke(
            bounds,
            0,
            Stroke::new(OVERLAY.stroke, OVERLAY.active),
            StrokeKind::Inside,
        );
    } else {
        ui.painter().rect_filled(bounds, 0, OVERLAY.dim);
    }
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
            Stroke::new(OVERLAY.stroke, Color32::WHITE),
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
    let hint_size = egui::Vec2::from(OVERLAY.hint_size);
    let hint = Rect::from_center_size(
        Pos2::new(
            bounds.center().x,
            bounds.min.y + OVERLAY.hint_top + hint_size.y / 2.0,
        ),
        hint_size,
    );
    if active && selection.start.is_none() {
        ui.painter()
            .rect_filled(hint, OVERLAY.hint_radius, OVERLAY.hint_fill);
        ui.painter().text(
            hint.center(),
            egui::Align2::CENTER_CENTER,
            "拖动框选区域，松手置顶 · Esc 取消",
            egui::FontId::proportional(OVERLAY.hint_font),
            Color32::WHITE,
        );
    }
    activate.then_some(Action::Activate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_screen_is_outlined_and_other_screens_are_dimmed() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "selection",
            egui::ColorImage::filled([2, 2], Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let mut selection = Selection {
            texture,
            pixels: [2, 2],
            start: None,
            focus_requested: false,
        };
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0));
        for active in [true, false] {
            let mut frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    ..Default::default()
                },
                |ui| {
                    show(ui, &mut selection, active);
                },
            );
            frame.textures_delta.clear();
            assert_eq!(frame.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == bounds && rect.stroke.color == Color32::from_rgb(56, 132, 255))
            }), active);
            assert_eq!(frame.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == bounds && rect.fill == Color32::from_black_alpha(28))
            }), !active);
            assert_eq!(
                frame.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .any(|command| { matches!(command, egui::ViewportCommand::Focus) }),
                active,
                "inactive displays must not steal keyboard focus"
            );
        }
        let mut frame = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                events: vec![egui::Event::PointerMoved(egui::pos2(50.0, 50.0))],
                ..Default::default()
            },
            |ui| {
                assert!(matches!(
                    show(ui, &mut selection, false),
                    Some(Action::Activate)
                ));
            },
        );
        frame.textures_delta.clear();
        assert!(frame.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == selection.texture.id())
        }), "switching displays must not clear the frozen desktop");
    }
}
