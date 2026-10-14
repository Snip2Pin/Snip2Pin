use std::sync::mpsc::Sender;

use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, ViewportCommand,
};

use crate::app::Event;
#[cfg(test)]
use snip2pin::geometry::Placement;

#[cfg(test)]
use snip2pin::pin::ToolbarSide;
use snip2pin::pin::{Coordinates, Drag};
pub use snip2pin::pin::{Frame, frame_geometry};

use super::{icons::paint_copy_icon, style::PIN};

#[cfg(test)]
fn toolbar_rects(image: Rect, below: bool) -> (Rect, Rect) {
    Frame {
        window: Rect::NOTHING,
        image,
        controls_area: Rect::NOTHING,
        grid: 1.0,
        toolbar_side: if below {
            ToolbarSide::Bottom
        } else {
            ToolbarSide::Top
        },
        toolbar_offset: egui::Vec2::ZERO,
        top_edge: false,
    }
    .toolbar_rects()
}

pub struct Model<'a> {
    pub id: u64,
    pub texture: &'a TextureHandle,
    pub frame: Frame,
    pub events: &'a Sender<Event>,
    pub monitors: &'a [Rect],
    pub coordinate_ratio: f32,
}

pub fn show(
    ui: &mut egui::Ui,
    model: Model<'_>,
    drag: &mut Drag,
    mut cursor: impl FnMut() -> Option<[f32; 2]>,
) {
    let Model {
        id,
        texture,
        frame,
        events,
        monitors,
        coordinate_ratio,
    } = model;
    let bounds = ui.max_rect();
    let image = frame.image;
    ui.painter()
        .add(PIN.shadow.as_shape(image, PIN.shadow_radius));
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
            Color32::from_white_alpha(PIN.border_alpha),
        ),
        StrokeKind::Outside,
    );
    let (copy_rect, close_rect) = frame.toolbar_rects();
    let response = ui.interact(image, ui.id().with("image-drag"), Sense::click_and_drag());
    let hovered = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|point| bounds.contains(point));
    let moved = ui.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerMoved(_)))
    });
    let pointer = if response.drag_started() || (response.dragged() && drag.is_active() && moved) {
        cursor()
    } else {
        None
    };
    let coordinates = Coordinates::new(1.0, ui.ctx().zoom_factor(), ui.ctx().pixels_per_point());
    if response.drag_started() {
        let native = ui.input(|input| input.viewport().inner_rect);
        if let Some(inner) = native
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
            && pointer.is_some()
        {
            drag.begin(
                press - image.min,
                image.min + inner.min.to_vec2(),
                &coordinates,
            );
        } else {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }
    if response.dragged()
        && drag.is_active()
        && moved
        && let Some(pointer) = pointer
        && let Some(position) = drag.move_to(pointer, &coordinates)
    {
        let (inner, outer) =
            ui.input(|input| (input.viewport().inner_rect, input.viewport().outer_rect));
        let inset = inner
            .zip(outer)
            .map_or(egui::Vec2::ZERO, |(inner, outer)| inner.min - outer.min);
        let proposed = position - frame.image.min.to_vec2() - inset;
        let origin = frame.drag_position(proposed, inset, monitors, coordinate_ratio);
        let image_origin = frame.image.min + (origin + inset).to_vec2();
        let moved =
            crate::platform::pin_host(frame.moved_to(image_origin, monitors, coordinate_ratio));
        // The adapter may bound the image at a system edge. Keep the final image
        // position so releasing the drag cannot restore obsolete upper margins.
        drag.set_position(moved.image.min + moved.window.min.to_vec2(), &coordinates);
        if inner.is_none_or(|inner| inner.size() != moved.window.size()) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::InnerSize(moved.window.size()));
        }
        let position = moved.window.min - inset;
        if outer.is_none_or(|outer| outer.min != position) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::OuterPosition(position));
        }
    }
    if !ui.input(|input| input.pointer.primary_down()) {
        drag.end();
    }
    if hovered {
        let copy = ui
            .put(copy_rect, egui::Button::new("").fill(PIN.button_fill))
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
                    .fill(PIN.button_fill),
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
mod tests;
