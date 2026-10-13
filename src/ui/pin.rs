use std::sync::mpsc::Sender;

use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Shadow, Stroke, StrokeKind, TextureHandle, ViewportCommand,
};

use crate::app::Event;
use snip2pin::geometry::Placement;

pub const PADDING: f32 = 10.0;

/// Snap the native window to its coordinate grid while retaining the image's
/// exact origin and size inside it. macOS windows use integral native points.
pub fn frame_geometry(placement: Placement, ratio: f32, grid: f32) -> (Rect, Rect) {
    let position = Pos2::new(placement.position[0], placement.position[1]) * ratio;
    let size = egui::vec2(placement.size[0], placement.size[1]) * ratio;
    let origin = Pos2::new(
        ((position.x - PADDING) * grid).floor() / grid,
        ((position.y - PADDING) * grid).floor() / grid,
    );
    let image = Rect::from_min_size((position - origin).to_pos2(), size);
    let window_size = egui::vec2(
        ((image.max.x + PADDING) * grid).ceil() / grid,
        ((image.max.y + PADDING) * grid).ceil() / grid,
    );
    (Rect::from_min_size(origin, window_size), image)
}

pub fn show(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    image: Rect,
    events: &Sender<Event>,
) {
    let bounds = ui.max_rect();
    ui.painter().add(
        Shadow {
            offset: [0, 2],
            blur: 12,
            spread: 1,
            color: Color32::from_black_alpha(90),
        }
        .as_shape(image, 2),
    );
    ui.painter().image(
        texture.id(),
        image,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    ui.painter().rect_stroke(
        image,
        0,
        Stroke::new(
            1.0 / ui.ctx().pixels_per_point(),
            Color32::from_white_alpha(190),
        ),
        StrokeKind::Outside,
    );
    let close_rect = Rect::from_min_size(
        Pos2::new((image.max.x - 30.0).max(bounds.min.x), image.min.y + 4.0),
        egui::vec2(26.0, 26.0),
    );
    let response = ui.interact(image, ui.id().with("image-drag"), Sense::click_and_drag());
    let hovered = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|point| bounds.contains(point));
    let on_close = ui
        .input(|input| input.pointer.interact_pos())
        .is_some_and(|point| close_rect.contains(point));
    if response.drag_started() && !on_close {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    let close = hovered
        && ui
            .put(
                close_rect,
                egui::Button::new(egui::RichText::new("×").color(Color32::WHITE))
                    .fill(Color32::from_black_alpha(160)),
            )
            .clicked();
    if close
        || response.double_clicked()
        || ui.input(|input| {
            input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
        })
    {
        let _ = events.send(Event::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn clicking_close_does_not_start_window_drag() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "pin",
            egui::ColorImage::filled([2, 2], Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let (sender, events) = mpsc::channel();
        let point = egui::pos2(193.0, 27.0);
        for input_events in [
            vec![egui::Event::PointerMoved(point)],
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            let mut frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(220.0, 140.0))),
                    events: input_events,
                    ..Default::default()
                },
                |ui| show(ui, 41, &texture, ui.max_rect().shrink(PADDING), &sender),
            );
            frame.textures_delta.clear();
            assert!(
                frame.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .all(|command| !matches!(command, ViewportCommand::StartDrag))
            );
        }
        assert!(matches!(events.try_recv(), Ok(Event::ClosePin(41))));
    }

    #[test]
    fn close_button_tracks_hover_and_pointer_leave_without_changing_image_bounds() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "pin",
            egui::ColorImage::filled([2, 2], Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let (sender, _events) = mpsc::channel();
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(220.0, 140.0));
        for (events, close_visible) in [
            (vec![], false),
            (
                vec![egui::Event::PointerMoved(egui::pos2(80.0, 60.0))],
                true,
            ),
            (vec![egui::Event::PointerGone], false),
        ] {
            let mut frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    events,
                    ..Default::default()
                },
                |ui| show(ui, 1, &texture, bounds.shrink(PADDING), &sender),
            );
            frame.textures_delta.clear();
            assert_eq!(
                frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "×")
                }),
                close_visible
            );
            let image = frame
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                        Some(mesh.calc_bounds())
                    }
                    _ => None,
                })
                .expect("image is painted");
            assert_eq!(image, bounds.shrink(PADDING));
            assert!(
                frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect) if rect.blur_width > 0.0)
                }),
                "the shadow is rendered outside the image"
            );
        }
    }
}
