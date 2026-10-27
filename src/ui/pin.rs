use eframe::egui::{
    self, Color32, Pos2, Rect, ResizeDirection, Sense, TextureHandle, ViewportCommand,
};
use snip2pin::pin::{Control, METRICS, MenuCommand, Toolbar, Zoom};

use super::{
    icons::{paint_copy_icon, paint_lock_icon},
    style::PIN,
};

mod input;
mod keyboard;
mod menu_input;
mod session;
pub use menu_input::MenuInput;

/// Release every shared UI state owned by a closed pin, in any host.
pub fn forget(ctx: &egui::Context, id: u64) {
    super::launcher::forget_pin(ctx, id);
    forget_input(ctx, id);
}

pub fn forget_input(ctx: &egui::Context, id: u64) {
    ctx.data_mut(|data| data.remove::<bool>(locked_press_key(id)));
    forget_adjustments(ctx, id);
}

fn locked_press_key(id: u64) -> egui::Id {
    // Only eligibility belongs to us; egui owns the press and drag episode.
    egui::Id::unique(("pin-locked-press", id))
}

fn forget_adjustments(ctx: &egui::Context, id: u64) {
    keyboard::forget(ctx, id);
    ctx.data_mut(|data| {
        data.remove::<session::Session>(session::key(id));
        if data
            .get_temp::<session::Owner>(session::owner_key())
            .is_some_and(|owner| owner.pin == id)
        {
            data.remove::<session::Owner>(session::owner_key());
        }
    });
}

/// Route a continuing gesture delivered to another window of this app.
/// Input delivered to other processes remains under the operating system.
pub fn route_gesture(ui: &egui::Ui, emit: &dyn Fn(Action)) {
    let owner = ui
        .ctx()
        .data(|data| data.get_temp::<session::Owner>(session::owner_key()));
    if let Some(owner) = owner
        && owner.viewport != ui.ctx().viewport_id()
    {
        input::adjust(ui, owner.pin, false, Interaction::default(), true, emit);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    UnlockAll,
    UnlockHint(u64),
    CopyPin(u64),
    ClosePin(u64),
    MenuPin(u64),
    ToggleLock(u64),
    Zoom(u64, Vec<Zoom>),
    Move(u64, [i32; 2]),
}

impl Action {
    pub fn from_menu(id: u64, command: MenuCommand) -> Self {
        match command {
            MenuCommand::Copy => Self::CopyPin(id),
            MenuCommand::ToggleLock => Self::ToggleLock(id),
            MenuCommand::RestoreSize => Self::Zoom(id, vec![Zoom::Original]),
            MenuCommand::Close => Self::ClosePin(id),
        }
    }
}

const MENU_PADDING: f32 = 8.0;
const MENU_ROWS: usize = MenuCommand::ALL.len();
pub const MENU_SIZE: egui::Vec2 = egui::vec2(
    PIN.menu_width,
    MENU_PADDING * 2.0
        + METRICS.button * MENU_ROWS as f32
        + METRICS.button_gap * (MENU_ROWS - 1) as f32,
);

#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction {
    pub native_edges: bool,
    pub locked: bool,
    pub toolbar: Option<Toolbar>,
    /// Virtual hosts supply per-image focus; native hosts use viewport focus.
    pub keyboard_focus: Option<bool>,
    pub space: InputSpace,
}

/// Host-observed image origin and native screen units per UI point.
#[derive(Clone, Copy, Debug)]
pub struct InputSpace {
    /// Unknown on hosts that cannot report a global window position.
    pub origin: Option<Pos2>,
    pub scale: f32,
}

impl Default for InputSpace {
    fn default() -> Self {
        Self {
            origin: None,
            scale: 1.0,
        }
    }
}

/// Paint one complete pin; hosts supply observed layout and handle native gestures.
pub fn show(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    interaction: Interaction,
    events: &dyn Fn(Action),
    host: impl FnMut(ViewportCommand),
) {
    let hovered = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|point| ui.max_rect().contains(point));
    image_with_host(ui, id, texture, interaction, events, host);
    if interaction
        .keyboard_focus
        .unwrap_or_else(|| ui.input(|input| input.viewport().focused == Some(true)))
    {
        ui.painter().rect_stroke(
            ui.max_rect(),
            0,
            egui::Stroke::new(
                PIN.highlight_stroke_pixels / ui.ctx().pixels_per_point(),
                PIN.highlight,
            ),
            egui::StrokeKind::Inside,
        );
    }
    if hovered && let Some(toolbar) = interaction.toolbar {
        ui.scope_builder(egui::UiBuilder::new().max_rect(toolbar.bounds), |ui| {
            controls(ui, id, interaction.locked, toolbar, events);
        });
    }
}

fn image_with_host(
    ui: &mut egui::Ui,
    id: u64,
    texture: &TextureHandle,
    interaction: Interaction,
    events: &dyn Fn(Action),
    mut host: impl FnMut(ViewportCommand),
) {
    let Interaction {
        native_edges,
        locked,
        toolbar,
        keyboard_focus,
        ..
    } = interaction;
    let excluded = toolbar.map(|toolbar| toolbar.bounds);
    let bounds = ui.max_rect();
    let focused =
        keyboard_focus.unwrap_or_else(|| ui.input(|input| input.viewport().focused == Some(true)));
    input::adjust(ui, id, focused, interaction, false, events);
    let resize_inset = if focused {
        PIN.focused_resize_inset
    } else {
        METRICS.inset
    };
    let response = ui.interact(bounds, ui.scope_id().with(("pin-image", id)), Sense::drag());
    if ui.is_enabled()
        && ui.ctx().current_pass_index() == 0
        && ui.input(|input| input.pointer.secondary_clicked())
        && ui
            .input(|input| input.pointer.interact_pos())
            .is_some_and(|point| {
                bounds.contains(point)
                    && ui.clip_rect().contains(point)
                    && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
            })
    {
        events(Action::MenuPin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    let edge = (!native_edges && !locked)
        .then(|| ui.input(|input| input.pointer.interact_pos()))
        .flatten()
        .filter(|point| !excluded.is_some_and(|rect| rect.contains(*point)))
        .and_then(|point| resize_direction(bounds, point, resize_inset));
    if response.hovered()
        && ui.is_enabled()
        && !native_edges
        && !locked
        && !ui
            .input(|input| input.pointer.interact_pos())
            .is_some_and(|point| excluded.is_some_and(|rect| rect.contains(point)))
    {
        if let Some(direction) = edge {
            ui.ctx().set_cursor_icon(resize_cursor(direction));
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    // Native drag/resize APIs need the initiating button press, before an egui
    // drag threshold turns it into a later mouse-move event.
    let pressed = ui.input(|input| {
        input.events.iter().rev().find_map(|event| match event {
            // press_origin is cleared when release arrives in the same frame.
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => Some(*pos),
            _ => None,
        })
    });
    let pressed_on_image = ui.is_enabled()
        && pressed.is_some_and(|point| {
            bounds.contains(point)
                && ui.clip_rect().contains(point)
                && ui.ctx().layer_id_at(point) == Some(ui.layer_id())
                && !toolbar.is_some_and(|toolbar| {
                    if locked {
                        // Preserve egui's hit tolerance for copy and unlock.
                        // Disabled close remains a blocked interaction attempt.
                        let radius = ui.ctx().global_style().interaction.interact_radius;
                        toolbar.controls().any(|(control, rect)| {
                            control != Control::Close && rect.expand(radius).contains(point)
                        })
                    } else {
                        toolbar.bounds.contains(point)
                    }
                })
        });
    if !locked && pressed_on_image && ui.input(|input| input.pointer.primary_down()) {
        let pressed_edge = (!native_edges)
            .then_some(pressed)
            .flatten()
            .and_then(|point| resize_direction(bounds, point, resize_inset));
        match pressed_edge {
            Some(direction) => host(ViewportCommand::BeginResize(direction)),
            None => host(ViewportCommand::StartDrag),
        }
    }
    ui.painter().image(
        texture.id(),
        bounds,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    if ui.ctx().current_pass_index() == 0 {
        let key = locked_press_key(id);
        let enabled = ui.is_enabled();
        let lost_focus = ui.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::WindowFocused(false)))
        });
        let primary_down = ui.input(|input| input.pointer.primary_down());
        let eligible = ui.ctx().data_mut(|data| {
            if !locked || !enabled || lost_focus {
                data.remove::<bool>(key);
                return false;
            }
            if pressed.is_some() {
                data.insert_temp(key, pressed_on_image);
            }
            let eligible = data.get_temp::<bool>(key).unwrap_or(false);
            if !primary_down {
                data.remove::<bool>(key);
            }
            eligible
        });
        let moved = primary_down
            && response.dragged_by(egui::PointerButton::Primary)
            && ui.input(|input| input.pointer.delta() != egui::Vec2::ZERO);
        if locked && !lost_focus && (pressed_on_image || (eligible && moved)) {
            events(Action::UnlockHint(id));
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
    if ui.ctx().current_pass_index() == 0 && !locked && close_requested(ui, focused) {
        events(Action::ClosePin(id));
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if locked && ui.input(|input| input.viewport().close_requested()) {
        host(ViewportCommand::CancelClose);
    }
}

fn controls(ui: &mut egui::Ui, id: u64, locked: bool, toolbar: Toolbar, events: &dyn Fn(Action)) {
    let locale = crate::i18n::locale(ui.ctx());
    for (control, rect) in toolbar.controls() {
        let command = match control {
            Control::Copy => MenuCommand::Copy,
            Control::Lock => MenuCommand::ToggleLock,
            Control::Close => MenuCommand::Close,
        };
        let enabled = command.enabled(locked);
        let response = ui
            .push_id(("pin-control", id, control), |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.put(
                        rect,
                        egui::Button::new(
                            egui::RichText::new(if control == Control::Close { "×" } else { "" })
                                .color(if enabled {
                                    Color32::WHITE
                                } else {
                                    Color32::GRAY
                                }),
                        )
                        .fill(PIN.button_fill),
                    )
                })
                .inner
            })
            .inner;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                ui.is_enabled() && enabled,
                locale.text(command.text(locked)),
            )
        });
        match control {
            Control::Copy => paint_copy_icon(ui, response.rect),
            Control::Lock => paint_lock_icon(ui, response.rect, locked, Color32::WHITE),
            Control::Close => {}
        }
        if response.clicked() {
            events(Action::from_menu(id, command));
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

pub fn menu(ui: &mut egui::Ui, id: u64, locked: bool, events: &dyn Fn(Action)) {
    let locale = crate::i18n::locale(ui.ctx());
    let bounds = ui.max_rect().shrink(MENU_PADDING);
    let height = (bounds.height() - METRICS.button_gap * (MENU_ROWS - 1) as f32) / MENU_ROWS as f32;
    for (index, command) in MenuCommand::ALL.into_iter().enumerate() {
        let rect = Rect::from_min_size(
            bounds.min + egui::vec2(0.0, index as f32 * (height + METRICS.button_gap)),
            egui::vec2(bounds.width(), height),
        );
        if ui
            .add_enabled_ui(command.enabled(locked), |ui| {
                ui.put(rect, egui::Button::new(locale.text(command.text(locked))))
            })
            .inner
            .clicked()
        {
            events(Action::from_menu(id, command));
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

fn close_requested(ui: &egui::Ui, focused: bool) -> bool {
    ui.is_enabled()
        && (ui.input(|input| input.viewport().close_requested())
            || (focused
                && !ui.ctx().text_edit_focused()
                && !egui::Popup::is_any_open(ui.ctx())
                && ui.input(|input| input.key_pressed(egui::Key::Escape))))
}

fn resize_direction(bounds: Rect, point: Pos2, inset: f32) -> Option<ResizeDirection> {
    if !bounds.contains(point) {
        return None;
    }
    let hit = egui::vec2(
        inset.min(bounds.width() / 3.0),
        inset.min(bounds.height() / 3.0),
    );
    let horizontal = if point.x < bounds.min.x + hit.x {
        -1
    } else if point.x > bounds.max.x - hit.x {
        1
    } else {
        0
    };
    let vertical = if point.y < bounds.min.y + hit.y {
        -1
    } else if point.y > bounds.max.y - hit.y {
        1
    } else {
        0
    };
    use ResizeDirection::*;
    match (horizontal, vertical) {
        (-1, -1) => Some(NorthWest),
        (0, -1) => Some(North),
        (1, -1) => Some(NorthEast),
        (-1, 0) => Some(West),
        (1, 0) => Some(East),
        (-1, 1) => Some(SouthWest),
        (0, 1) => Some(South),
        (1, 1) => Some(SouthEast),
        _ => None,
    }
}

fn resize_cursor(direction: ResizeDirection) -> egui::CursorIcon {
    use ResizeDirection::*;
    match direction {
        North | South => egui::CursorIcon::ResizeVertical,
        West | East => egui::CursorIcon::ResizeHorizontal,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
    }
}

#[cfg(test)]
mod tests;
