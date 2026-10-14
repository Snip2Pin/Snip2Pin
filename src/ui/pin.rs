use std::sync::mpsc::Sender;

use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Shadow, Stroke, StrokeKind, TextureHandle, ViewportCommand,
};

use crate::app::Event;
use snip2pin::geometry::Placement;

const PADDING: f32 = 10.0;
const BUTTON_HEIGHT: f32 = 26.0;
const COPY_WIDTH: f32 = BUTTON_HEIGHT;
const BUTTON_GAP: f32 = 4.0;
const IMAGE_GAP: f32 = 6.0;
const TOOLBAR_WIDTH: f32 = COPY_WIDTH + BUTTON_GAP + BUTTON_HEIGHT;

#[derive(Clone, Copy)]
pub struct Frame {
    pub window: Rect,
    pub image: Rect,
    pub toolbar_below: bool,
    pub top_edge: bool,
}

/// Keep image dimensions unchanged. Top-edge pins move down to leave room for
/// controls, preferring a lower toolbar whenever it fits on the display.
pub fn frame_geometry(placement: Placement, screen: Rect, ratio: f32, grid: f32) -> Frame {
    let mut position = Pos2::new(placement.position[0], placement.position[1]) * ratio;
    let size = egui::vec2(placement.size[0], placement.size[1]) * ratio;
    let screen_top = screen.min.y * ratio;
    let top_edge = position.y - PADDING - BUTTON_HEIGHT - IMAGE_GAP < screen_top;
    let mut toolbar_below = top_edge;
    if top_edge {
        position.y = position.y.max((screen_top * grid).ceil() / grid + PADDING);
        if position.y + size.y + PADDING + BUTTON_HEIGHT + IMAGE_GAP > screen.max.y * ratio {
            // Keep tall images full-sized, moving down to make a visible row
            // above instead of placing the lower controls beyond the display.
            toolbar_below = false;
            position.y = position
                .y
                .max((screen_top * grid).ceil() / grid + PADDING + BUTTON_HEIGHT + IMAGE_GAP);
        }
    }
    let origin = Pos2::new(
        ((position.x - PADDING) * grid).floor() / grid,
        ((position.y
            - PADDING
            - if toolbar_below {
                0.0
            } else {
                BUTTON_HEIGHT + IMAGE_GAP
            })
            * grid)
            .floor()
            / grid,
    );
    let image = Rect::from_min_size((position - origin).to_pos2(), size);
    let window_size = egui::vec2(
        ((image.min.x + size.x.max(TOOLBAR_WIDTH) + PADDING) * grid).ceil() / grid,
        ((image.max.y
            + PADDING
            + if toolbar_below {
                BUTTON_HEIGHT + IMAGE_GAP
            } else {
                0.0
            })
            * grid)
            .ceil()
            / grid,
    );
    Frame {
        window: Rect::from_min_size(origin, window_size),
        image,
        toolbar_below,
        top_edge,
    }
}

fn toolbar_rects(image: Rect, below: bool) -> (Rect, Rect) {
    let close = Rect::from_min_size(
        Pos2::new(
            image.max.x.max(image.min.x + TOOLBAR_WIDTH) - BUTTON_HEIGHT,
            if below {
                image.max.y + IMAGE_GAP
            } else {
                image.min.y - IMAGE_GAP - BUTTON_HEIGHT
            },
        ),
        egui::vec2(BUTTON_HEIGHT, BUTTON_HEIGHT),
    );
    let copy = Rect::from_min_size(
        Pos2::new(close.min.x - BUTTON_GAP - COPY_WIDTH, close.min.y),
        egui::vec2(COPY_WIDTH, BUTTON_HEIGHT),
    );
    (copy, close)
}

fn paint_copy_icon(ui: &egui::Ui, rect: Rect) {
    let origin = rect.center() - egui::vec2(8.0, 8.0);
    let point = |x, y| origin + egui::vec2(x, y) * (16.0 / 24.0);
    let stroke = Stroke::new(1.7 * (16.0 / 24.0), Color32::WHITE);
    ui.painter().add(egui::Shape::line(
        [
            (5.0, 15.0),
            (4.0, 15.0),
            (2.6, 14.4),
            (2.0, 13.0),
            (2.0, 5.0),
            (2.6, 3.6),
            (4.0, 3.0),
            (15.0, 3.0),
            (16.4, 3.6),
            (17.0, 5.0),
            (17.0, 6.0),
        ]
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect(),
        stroke,
    ));
    ui.painter().rect_stroke(
        Rect::from_min_max(point(7.0, 8.0), point(22.0, 21.0)),
        1,
        stroke,
        StrokeKind::Middle,
    );
    ui.painter().add(egui::Shape::line(
        [
            (8.0, 19.0),
            (12.0, 15.0),
            (15.0, 18.0),
            (17.0, 16.0),
            (21.0, 20.0),
        ]
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect(),
        stroke,
    ));
    ui.painter()
        .circle_filled(point(17.5, 12.5), 16.0 / 24.0, Color32::WHITE);
}

pub fn show(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    frame: Frame,
    events: &Sender<Event>,
) {
    let bounds = ui.max_rect();
    let image = frame.image;
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
    let (copy_rect, close_rect) = toolbar_rects(image, frame.toolbar_below);
    let response = ui.interact(image, ui.id().with("image-drag"), Sense::click_and_drag());
    let hovered = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|point| bounds.contains(point));
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if hovered {
        let copy = ui
            .put(
                copy_rect,
                egui::Button::new("").fill(Color32::from_black_alpha(160)),
            )
            .on_hover_text("复制");
        copy.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "复制")
        });
        paint_copy_icon(ui, copy.rect);
        if copy.clicked() {
            let _ = events.send(Event::CopyPin(id));
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
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
                let frame = frame_geometry(
                    placement,
                    Rect::from_min_size(egui::pos2(-1000.0, -1000.0), egui::vec2(3000.0, 3000.0)),
                    ratio,
                    grid,
                );
                let image = frame.image;
                let (copy, close) = toolbar_rects(image, frame.toolbar_below);
                let bounds = Rect::from_min_size(Pos2::ZERO, frame.window.size());
                assert!(bounds.contains_rect(copy) && bounds.contains_rect(close));
                assert!(copy.max.x < close.min.x);
                assert!(copy.max.y < image.min.y && close.max.y < image.min.y);
                assert!(
                    (frame.window.min + image.min.to_vec2())
                        .distance(Pos2::new(placement.position[0], placement.position[1]) * ratio)
                        < 0.001
                );
                assert!((image.size() - egui::vec2(size[0], size[1]) * ratio).length() < 0.001);
            }
        }
    }

    #[test]
    fn top_edge_layout_preserves_original_dimensions_and_keeps_controls_visible() {
        for origin in [egui::pos2(0.0, 28.0), egui::pos2(-680.0, -172.0)] {
            for ratio in [1.0, 0.8] {
                for size in [[12.0, 8.0], [193.0, 121.0], [680.0, 420.0]] {
                    let screen = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
                    let placement = Placement {
                        position: [
                            origin.x + if size[0] < 680.0 { 80.0 } else { 0.0 },
                            origin.y - 28.0,
                        ],
                        size,
                    };
                    let frame = frame_geometry(placement, screen, ratio, 1.0 / ratio);
                    let (copy, close) = toolbar_rects(frame.image, frame.toolbar_below);
                    assert!(frame.top_edge);
                    assert!(frame.window.min.y >= screen.min.y * ratio);
                    let bounds = Rect::from_min_size(Pos2::ZERO, frame.window.size());
                    assert!(bounds.contains_rect(copy) && bounds.contains_rect(close));
                    let visible = Rect::from_min_max(screen.min * ratio, screen.max * ratio);
                    assert!(visible.contains_rect(copy.translate(frame.window.min.to_vec2())));
                    assert!(visible.contains_rect(close.translate(frame.window.min.to_vec2())));
                    assert!(!copy.intersects(frame.image) && !close.intersects(frame.image));
                    assert_eq!(frame.toolbar_below, size[1] < 200.0);
                    assert!((frame.image.aspect_ratio() - size[0] / size[1]).abs() < 0.001);
                    assert!(
                        (frame.image.size() - egui::vec2(size[0], size[1]) * ratio).length()
                            < 0.001,
                        "even tall captures must keep their original dimensions"
                    );
                }
            }
        }
    }

    #[test]
    fn clicking_toolbar_buttons_dispatches_actions_without_window_drag() {
        for (copy, below) in [(true, false), (false, false), (true, true), (false, true)] {
            let ctx = egui::Context::default();
            let texture = ctx.load_texture(
                "pin",
                egui::ColorImage::filled([2, 2], Color32::WHITE),
                egui::TextureOptions::NEAREST,
            );
            let (sender, events) = mpsc::channel();
            let image = Rect::from_min_size(egui::pos2(10.0, 42.0), egui::vec2(200.0, 88.0));
            let frame = Frame {
                window: Rect::from_min_size(Pos2::ZERO, egui::vec2(220.0, 140.0)),
                image,
                toolbar_below: below,
                top_edge: false,
            };
            let (copy_rect, close_rect) = toolbar_rects(image, below);
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
                            egui::vec2(220.0, 180.0),
                        )),
                        events: input_events,
                        ..Default::default()
                    },
                    |ui| show(ui, 41, &texture, frame, &sender),
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
        let layout = Frame {
            window: bounds,
            image: image_rect,
            toolbar_below: false,
            top_edge: false,
        };
        for (events, close_visible) in [
            (vec![], false),
            (
                vec![egui::Event::PointerMoved(egui::pos2(80.0, 60.0))],
                true,
            ),
            (
                vec![egui::Event::PointerMoved(
                    toolbar_rects(image_rect, false).0.center(),
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
                |ui| show(ui, 1, &texture, layout, &sender),
            );
            frame.textures_delta.clear();
            assert_eq!(
                frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "×")
                }),
                close_visible
            );
            assert_eq!(
                frame.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Path(path) if path.points.len() == 5)),
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
