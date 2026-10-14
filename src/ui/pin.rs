use std::sync::mpsc::Sender;

use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Shadow, Stroke, StrokeKind, TextureHandle, ViewportCommand,
};

use crate::app::Event;
use snip2pin::geometry::Placement;

pub const PADDING: f32 = 10.0;
const BUTTON_HEIGHT: f32 = 26.0;
const COPY_WIDTH: f32 = 48.0;
const BUTTON_GAP: f32 = 4.0;
const IMAGE_GAP: f32 = 6.0;
const TOOLBAR_WIDTH: f32 = COPY_WIDTH + BUTTON_GAP + BUTTON_HEIGHT;

/// Snap the native window to its coordinate grid while retaining the image's
/// exact origin and size inside it. macOS windows use integral native points.
pub fn frame_geometry(placement: Placement, ratio: f32, grid: f32) -> (Rect, Rect) {
    let position = Pos2::new(placement.position[0], placement.position[1]) * ratio;
    let size = egui::vec2(placement.size[0], placement.size[1]) * ratio;
    let origin = Pos2::new(
        ((position.x - PADDING - (TOOLBAR_WIDTH - size.x).max(0.0)) * grid).floor() / grid,
        ((position.y - PADDING - BUTTON_HEIGHT - IMAGE_GAP) * grid).floor() / grid,
    );
    let image = Rect::from_min_size((position - origin).to_pos2(), size);
    let window_size = egui::vec2(
        ((image.max.x + PADDING) * grid).ceil() / grid,
        ((image.max.y + PADDING) * grid).ceil() / grid,
    );
    (Rect::from_min_size(origin, window_size), image)
}

fn toolbar_rects(image: Rect) -> (Rect, Rect) {
    let close = Rect::from_min_size(
        Pos2::new(
            image.max.x - BUTTON_HEIGHT,
            image.min.y - IMAGE_GAP - BUTTON_HEIGHT,
        ),
        egui::vec2(BUTTON_HEIGHT, BUTTON_HEIGHT),
    );
    let copy = Rect::from_min_size(
        Pos2::new(close.min.x - BUTTON_GAP - COPY_WIDTH, close.min.y),
        egui::vec2(COPY_WIDTH, BUTTON_HEIGHT),
    );
    (copy, close)
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
    let (copy_rect, close_rect) = toolbar_rects(image);
    let response = ui.interact(image, ui.id().with("image-drag"), Sense::click_and_drag());
    let hovered = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|point| bounds.contains(point));
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if hovered
        && ui
            .put(
                copy_rect,
                egui::Button::new(egui::RichText::new("复制").color(Color32::WHITE))
                    .fill(Color32::from_black_alpha(160)),
            )
            .clicked()
    {
        let _ = events.send(Event::CopyPin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
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
    fn toolbar_fits_tiny_images_without_changing_their_desktop_bounds() {
        for size in [[1.0, 1.0], [12.5, 7.5], [193.0, 121.0]] {
            for (ratio, grid) in [(1.0, 1.0), (0.8, 1.25), (1.0, 2.0)] {
                let placement = Placement {
                    position: [-680.5, 100.25],
                    size,
                };
                let (window, image) = frame_geometry(placement, ratio, grid);
                let (copy, close) = toolbar_rects(image);
                let bounds = Rect::from_min_size(Pos2::ZERO, window.size());
                assert!(bounds.contains_rect(copy) && bounds.contains_rect(close));
                assert!(copy.max.x < close.min.x);
                assert!(copy.max.y < image.min.y && close.max.y < image.min.y);
                assert!(
                    (window.min + image.min.to_vec2())
                        .distance(Pos2::new(placement.position[0], placement.position[1]) * ratio)
                        < 0.001
                );
                assert!((image.size() - egui::vec2(size[0], size[1]) * ratio).length() < 0.001);
            }
        }
    }

    #[test]
    fn clicking_toolbar_buttons_dispatches_actions_without_window_drag() {
        for copy in [true, false] {
            let ctx = egui::Context::default();
            let texture = ctx.load_texture(
                "pin",
                egui::ColorImage::filled([2, 2], Color32::WHITE),
                egui::TextureOptions::NEAREST,
            );
            let (sender, events) = mpsc::channel();
            let image = Rect::from_min_size(egui::pos2(10.0, 42.0), egui::vec2(200.0, 88.0));
            let (copy_rect, close_rect) = toolbar_rects(image);
            let point = if copy {
                copy_rect.center()
            } else {
                close_rect.center()
            };
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
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(220.0, 140.0),
                        )),
                        events: input_events,
                        ..Default::default()
                    },
                    |ui| show(ui, 41, &texture, image, &sender),
                );
                frame.textures_delta.clear();
                assert!(
                    frame.viewport_output[&egui::ViewportId::ROOT]
                        .commands
                        .iter()
                        .all(|command| !matches!(command, ViewportCommand::StartDrag))
                );
            }
            if copy {
                assert!(matches!(events.try_recv(), Ok(Event::CopyPin(41))));
            } else {
                assert!(matches!(events.try_recv(), Ok(Event::ClosePin(41))));
            }
            assert!(events.try_recv().is_err());
        }
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
        let image_rect = Rect::from_min_size(egui::pos2(10.0, 42.0), egui::vec2(200.0, 88.0));
        for (events, close_visible) in [
            (vec![], false),
            (
                vec![egui::Event::PointerMoved(egui::pos2(80.0, 60.0))],
                true,
            ),
            (
                vec![egui::Event::PointerMoved(
                    toolbar_rects(image_rect).0.center(),
                )],
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
                |ui| show(ui, 1, &texture, image_rect, &sender),
            );
            frame.textures_delta.clear();
            assert_eq!(
                frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "×")
                }),
                close_visible
            );
            assert_eq!(
                frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "复制")
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
            assert_eq!(image, image_rect);
            assert!(
                frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect) if rect.blur_width > 0.0)
                }),
                "the shadow is rendered outside the image"
            );
        }
    }
}
