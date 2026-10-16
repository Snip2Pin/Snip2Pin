use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use snip2pin::geometry::{self, PixelRect};

use super::style::OVERLAY;

pub struct Selection {
    pub texture: TextureHandle,
    pub pixels: [u32; 2],
    /// Source pixels per native window coordinate step, independent of UI zoom.
    pub pixel_step: [u32; 2],
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
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        return Some(Action::Cancel);
    }
    if response.drag_started() {
        selection.start = ui.input(|input| input.pointer.press_origin());
    }
    let current = ui.input(|input| input.pointer.interact_pos());
    if let (Some(start), Some(current)) = (selection.start, current) {
        let selected = geometry::selection_with_step(
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
            selection.pixel_step,
        );
        if let Some(pixels) = selected {
            let units =
                bounds.size() / egui::vec2(selection.pixels[0] as f32, selection.pixels[1] as f32);
            let rect = Rect::from_min_size(
                bounds.min + egui::vec2(pixels.x as f32, pixels.y as f32) * units,
                egui::vec2(pixels.width as f32, pixels.height as f32) * units,
            );
            ui.painter().rect_stroke(
                rect,
                0,
                Stroke::new(OVERLAY.stroke, Color32::WHITE),
                StrokeKind::Inside,
            );
        }
        if response.drag_stopped() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            selection.start = None;
            return selected.map(Action::Select);
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
    fn preview_outline_and_released_crop_share_the_native_pixel_grid() {
        let pixels = [2000, 1600];
        for (start, end, expected) in [
            (
                [100.5, 150.5],
                [400.5, 350.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [400.5, 350.5],
                [100.5, 150.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [400.5, 150.5],
                [100.5, 350.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [100.5, 350.5],
                [400.5, 150.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [500.5, 600.5],
                [1100.0, 900.0],
                PixelRect {
                    x: 1000,
                    y: 1200,
                    width: 1000,
                    height: 400,
                },
            ),
            (
                [100.5, 150.5],
                [-20.0, -30.0],
                PixelRect {
                    x: 0,
                    y: 0,
                    width: 202,
                    height: 302,
                },
            ),
        ] {
            for zoom in [1.0, 1.25] {
                let ctx = egui::Context::default();
                let bounds =
                    Rect::from_min_size(egui::pos2(13.0, 17.0), egui::vec2(1000.0, 800.0) / zoom);
                let start = bounds.min + egui::Vec2::from(start) / zoom;
                let end = bounds.min + egui::Vec2::from(end) / zoom;
                let mut selection = Selection {
                    texture: ctx.load_texture(
                        "generated selection",
                        egui::ColorImage::filled([2, 2], Color32::BLACK),
                        egui::TextureOptions::NEAREST,
                    ),
                    pixels,
                    pixel_step: [2, 2],
                    start: None,
                    focus_requested: true,
                };
                let button = |pos, pressed| egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                let frames = [
                    vec![egui::Event::PointerMoved(start)],
                    vec![button(start, true)],
                    vec![egui::Event::PointerMoved(end)],
                    vec![button(end, false)],
                ];
                let expected_outline = Rect::from_min_size(
                    bounds.min + egui::vec2(expected.x as f32, expected.y as f32) / (2.0 * zoom),
                    egui::vec2(expected.width as f32, expected.height as f32) / (2.0 * zoom),
                );
                for (frame, events) in frames.into_iter().enumerate() {
                    let mut output = ctx.run_ui(egui::RawInput {
                        screen_rect: Some(Rect::from_min_max(Pos2::ZERO, bounds.max)),
                        events,
                        ..Default::default()
                    }, |ui| {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            let action = show(ui, &mut selection, true);
                            if frame == 3 {
                                assert!(matches!(action, Some(Action::Select(rect)) if rect == expected), "released crop must match source grid, zoom={zoom}");
                            }
                        });
                    });
                    output.textures_delta.clear();
                    if frame >= 2 {
                        let outline = output
                            .shapes
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Rect(rect) if rect.stroke.color == Color32::WHITE => {
                                    Some(rect.rect)
                                }
                                _ => None,
                            })
                            .expect("selection outline");
                        assert!(outline.min.distance(expected_outline.min) < 0.001);
                        assert!(outline.max.distance(expected_outline.max) < 0.001);
                    }
                }
            }
        }
    }

    #[test]
    fn completing_or_cancelling_selection_restores_the_cursor() {
        for cancel in [false, true] {
            let ctx = egui::Context::default();
            let texture = ctx.load_texture(
                "cursor",
                egui::ColorImage::filled([2, 2], Color32::WHITE),
                egui::TextureOptions::NEAREST,
            );
            let mut selection = Selection {
                texture,
                pixels: [640, 480],
                pixel_step: [1, 1],
                start: None,
                focus_requested: false,
            };
            let start = egui::pos2(40.0, 60.0);
            let end = egui::pos2(220.0, 180.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let last_event = if cancel {
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }
            } else {
                button(end, false)
            };
            let frames = [
                vec![egui::Event::PointerMoved(start)],
                vec![button(start, true)],
                vec![egui::Event::PointerMoved(end)],
                vec![last_event],
            ];
            for (index, events) in frames.into_iter().enumerate() {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(640.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let action = show(ui, &mut selection, true);
                        if index == 3 {
                            assert!(matches!(
                                (&action, cancel),
                                (Some(Action::Cancel), true) | (Some(Action::Select(_)), false)
                            ));
                        }
                    },
                );
                output.textures_delta.clear();
                assert_eq!(
                    output.platform_output.cursor_icon,
                    if index == 3 {
                        egui::CursorIcon::Default
                    } else {
                        egui::CursorIcon::Crosshair
                    }
                );
            }
        }
    }

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
            pixel_step: [1, 1],
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
