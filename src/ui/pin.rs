use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, ViewportCommand,
};

#[cfg(test)]
use snip2pin::geometry::Placement;

#[cfg(test)]
use snip2pin::pin::Prepared;
#[cfg(test)]
use snip2pin::pin::ToolbarSide;
use snip2pin::pin::{Coordinates, Preparation, ResizeHandle};
pub use snip2pin::pin::{Frame, frame_geometry};

use super::{icons::paint_copy_icon, style::PIN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    CopyPin(u64),
    ClosePin(u64),
}

#[derive(Clone, Copy)]
pub enum Surface {
    Native,
    Embedded,
}

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
    pub events: &'a dyn Fn(Action),
    pub monitors: &'a [Rect],
    pub coordinate_ratio: f32,
    pub minimum_size: egui::Vec2,
    pub surface: Surface,
}

pub fn show(
    ui: &mut egui::Ui,
    model: Model<'_>,
    prepared: &mut Preparation,
    cursor: impl FnMut() -> Option<[f32; 2]>,
) {
    show_with_host(ui, model, prepared, cursor, |_| None);
}

/// A native adapter may commit geometry before shapes reach the renderer.
pub fn show_with_host(
    ui: &mut egui::Ui,
    model: Model<'_>,
    prepared: &mut Preparation,
    mut cursor: impl FnMut() -> Option<[f32; 2]>,
    mut layout: impl FnMut(Frame) -> Option<Rect>,
) {
    let Model {
        id,
        texture,
        mut frame,
        events,
        monitors,
        coordinate_ratio,
        minimum_size,
        surface,
    } = model;
    let (copy, close) = frame.toolbar_rects();
    let bounds = frame.image.expand(PIN.resize_hit).union(copy).union(close);
    let image = frame.image;
    let response = ui
        .interact(image, ui.id().with("image-drag"), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::Grab)
        .on_hover_text("拖动移动 · 双击关闭");
    let hovered = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|point| bounds.contains(point));
    let handles = ResizeHandle::ALL
        .into_iter()
        .map(|corner| {
            let hit = corner.hit_rect(image, PIN.resize_hit);
            let response = ui
                .interact(
                    hit,
                    ui.id().with(("resize", corner)),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(corner.cursor())
                .on_hover_text("拖动缩放 · Shift 等比");
            (corner, response)
        })
        .collect::<Vec<_>>();
    let resize_started = handles
        .iter()
        .find_map(|(corner, response)| response.drag_started().then_some(*corner));
    let moved = ui.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerMoved(_)))
    });
    let pointer = if resize_started.is_some()
        || response.drag_started()
        || (moved
            && (prepared.resize.is_active() || (response.dragged() && prepared.drag.is_active())))
    {
        cursor()
    } else {
        None
    };
    let coordinates = Coordinates::new(1.0, ui.ctx().zoom_factor(), ui.ctx().pixels_per_point());
    let host = ui
        .input(|input| input.viewport().inner_rect)
        .unwrap_or(frame.window);
    if let Some(corner) = resize_started
        && let Some(press) = ui.input(|input| input.pointer.press_origin())
    {
        let desktop_image = image.translate(host.min.to_vec2());
        prepared.drag.end();
        prepared.resize.begin(
            corner,
            Rect::from_min_max(
                desktop_image.min * coordinates.grid,
                desktop_image.max * coordinates.grid,
            ),
            (press + host.min.to_vec2()) * coordinates.grid,
        );
    }
    if prepared.resize.is_active()
        && moved
        && let Some(pointer) = pointer.map(Pos2::from).or_else(|| {
            // Compositor-controlled desktops may not expose a global cursor.
            // Local pointer events still support user-directed resizing.
            ui.input(|input| input.pointer.interact_pos())
                .map(|point| (point + host.min.to_vec2()) * coordinates.grid)
        })
        && let Some(resized) = prepared.resize.update(
            pointer,
            ui.input(|input| input.modifiers.shift),
            minimum_size * coordinates.grid,
            cfg!(target_os = "macos").then_some(frame.controls_area.min.y * coordinates.grid),
        )
    {
        let image = Rect::from_min_max(
            resized.min / coordinates.grid,
            resized.max / coordinates.grid,
        );
        let next = snip2pin::pin::host_frame(frame.resized_to(image, monitors, coordinate_ratio));
        frame = apply_frame(ui, next, prepared, surface, &mut layout);
    }
    if response.drag_started() && !prepared.resize.is_active() {
        let native = ui
            .input(|input| input.viewport().inner_rect)
            .or_else(|| matches!(surface, Surface::Embedded).then_some(frame.window));
        if let Some(inner) = native
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
            && pointer.is_some()
        {
            prepared.drag.begin(
                press - image.min,
                image.min + inner.min.to_vec2(),
                &coordinates,
            );
        } else {
            prepared.begin_system_drag(host.min * coordinates.grid);
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }
    if response.dragged()
        && prepared.drag.is_active()
        && !prepared.resize.is_active()
        && moved
        && let Some(pointer) = pointer
        && let Some(position) = prepared.drag.move_to(pointer, &coordinates)
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
            snip2pin::pin::host_frame(frame.moved_to(image_origin, monitors, coordinate_ratio));
        frame = apply_frame(ui, moved, prepared, surface, &mut layout);
    }
    if !ui.input(|input| input.pointer.primary_down()) {
        prepared.drag.end();
        prepared.resize.end();
        prepared.end_system_drag();
    }
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
    if hovered {
        for corner in ResizeHandle::CORNERS {
            let point = corner.point(image);
            let delta = image.center() - point;
            let inward = egui::vec2(delta.x.signum(), delta.y.signum());
            let stroke = Stroke::new(1.0, Color32::WHITE);
            ui.painter().line_segment(
                [point, point + egui::vec2(inward.x * PIN.resize_mark, 0.0)],
                stroke,
            );
            ui.painter().line_segment(
                [point, point + egui::vec2(0.0, inward.y * PIN.resize_mark)],
                stroke,
            );
        }
    }
    let (copy_rect, close_rect) = frame.toolbar_rects();
    if hovered {
        let copy = ui
            .put(copy_rect, egui::Button::new("").fill(PIN.button_fill))
            .on_hover_text("复制");
        copy.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "复制")
        });
        paint_copy_icon(ui, copy.rect);
        if copy.clicked() {
            events(Action::CopyPin(id));
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
        events(Action::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
}

fn apply_frame(
    ui: &mut egui::Ui,
    frame: Frame,
    prepared: &mut Preparation,
    surface: Surface,
    layout: &mut impl FnMut(Frame) -> Option<Rect>,
) -> Frame {
    prepared.commit(frame);
    if matches!(surface, Surface::Embedded) {
        let mut embedded = frame;
        embedded.image = frame.image.translate(frame.window.min.to_vec2());
        embedded.window = ui.max_rect();
        return embedded;
    }
    let image = frame.image.translate(frame.window.min.to_vec2());
    if let Some(actual) = layout(frame) {
        ui.set_clip_rect(Rect::from_min_size(Pos2::ZERO, actual.size()));
        let mut painted = frame;
        painted.window = actual;
        painted.image = image.translate(-actual.min.to_vec2());
        return painted;
    }
    let (inner, outer) =
        ui.input(|input| (input.viewport().inner_rect, input.viewport().outer_rect));
    let inset = inner
        .zip(outer)
        .map_or(egui::Vec2::ZERO, |(inner, outer)| inner.min - outer.min);
    if inner.is_none_or(|inner| inner.size() != frame.window.size()) {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::InnerSize(frame.window.size()));
    }
    let position = frame.window.min - inset;
    if outer.is_none_or(|outer| outer.min != position) {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::OuterPosition(position));
    }
    ui.ctx().request_repaint();
    let mut painted = frame;
    let observed = inner.unwrap_or(frame.window);
    painted.window = observed;
    painted.image = image.translate(-observed.min.to_vec2());
    painted
}

#[cfg(test)]
mod tests;
