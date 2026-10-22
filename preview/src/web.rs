use std::{
    cell::RefCell,
    sync::{Arc, mpsc},
};

use eframe::egui::{
    self, ColorImage, Pos2, Rect, ResizeDirection, TextureHandle, Vec2, ViewportCommand,
};
use snip2pin::{
    capture, geometry,
    i18n::{self, Locale, Message, Text},
    pin,
    session::CaptureGate,
    settings::Preferences,
    ui::{launcher, overlay, pin as pin_view, shortcut},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, prelude::*};

use crate::desktop;

struct Pin {
    id: u64,
    name: String,
    texture: TextureHandle,
    image: Rect,
    gesture: Option<Gesture>,
    locked: bool,
}

/// Freeze pixels and their overlay for exactly one selection session.
struct FrozenCapture {
    pixels: Arc<ColorImage>,
    selection: overlay::Selection,
}

/// Browser-only stand-in for a desktop window manager. Native builds never use
/// this geometry path, and the sandbox cannot validate native live resizing.
struct Gesture {
    origin: Pos2,
    image: Rect,
    direction: Option<ResizeDirection>,
}

impl Gesture {
    fn image_at(&self, pointer: Pos2, maximum: Vec2, pixels_per_point: f32) -> Rect {
        let delta = pointer - self.origin;
        let Some(direction) = self.direction else {
            return self.image.translate(delta);
        };
        use ResizeDirection::*;
        // The browser stand-in needs one canvas pixel to avoid inverted Areas.
        let minimum = Vec2::splat(pixels_per_point.recip());
        let maximum = maximum.max(minimum);
        let mut image = self.image;
        if matches!(direction, West | NorthWest | SouthWest) {
            image.min.x = (self.image.min.x + delta.x)
                .clamp(self.image.max.x - maximum.x, self.image.max.x - minimum.x);
        }
        if matches!(direction, East | NorthEast | SouthEast) {
            image.max.x = (self.image.max.x + delta.x)
                .clamp(self.image.min.x + minimum.x, self.image.min.x + maximum.x);
        }
        if matches!(direction, North | NorthWest | NorthEast) {
            image.min.y = (self.image.min.y + delta.y)
                .clamp(self.image.max.y - maximum.y, self.image.max.y - minimum.y);
        }
        if matches!(direction, South | SouthWest | SouthEast) {
            image.max.y = (self.image.max.y + delta.y)
                .clamp(self.image.min.y + minimum.y, self.image.min.y + maximum.y);
        }
        image
    }
}

struct Sandbox {
    preferences: Preferences,
    recording: shortcut::Recorder,
    startup: bool,
    message: Option<Message>,
    system_locale: Locale,
    wallpaper: TextureHandle,
    scene: desktop::Scene,
    capture_gate: CaptureGate,
    capture_result: Option<mpsc::Receiver<Arc<ColorImage>>>,
    window_rects: Vec<(egui::LayerId, Rect)>,
    desktop_bounds: Rect,
    icon: TextureHandle,
    applied_icon: snip2pin::icon::Variant,
    selection: Option<FrozenCapture>,
    pins: Vec<Pin>,
    menu: Option<(u64, Pos2)>,
    unlock_hint: Option<(u64, f64)>,
    next_id: u64,
    // Simulated native focus; browser Areas do not have OS window focus.
    focused_pin: Option<u64>,
}

impl Sandbox {
    fn new(ctx: &egui::Context, font: Vec<u8>, wallpaper: ColorImage) -> Self {
        if !font.is_empty() {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "sandbox-cjk".into(),
                Arc::new(egui::FontData::from_owned(font)),
            );
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.insert(0, "sandbox-cjk".into());
            }
            ctx.set_fonts(fonts);
        }
        let preferences = Preferences::default();
        let applied_icon = preferences.icon_variant(ctx.system_theme());
        Self {
            preferences,
            recording: shortcut::Recorder::default(),
            startup: false,
            message: None,
            system_locale: Locale::system(),
            wallpaper: ctx.load_texture("preview-wallpaper", wallpaper, Default::default()),
            scene: desktop::Scene::default(),
            capture_gate: CaptureGate::default(),
            capture_result: None,
            window_rects: Vec::new(),
            desktop_bounds: Rect::NOTHING,
            icon: ctx.load_texture(
                "brand",
                snip2pin::icon::render(applied_icon, snip2pin::icon::WINDOW_SIZE)
                    .map(|pixels| {
                        ColorImage::from_rgba_unmultiplied(
                            [pixels.width() as usize, pixels.height() as usize],
                            pixels.as_raw(),
                        )
                    })
                    .unwrap_or_else(|_| ColorImage::filled([1, 1], egui::Color32::TRANSPARENT)),
                Default::default(),
            ),
            applied_icon,
            selection: None,
            pins: Vec::new(),
            menu: None,
            unlock_hint: None,
            next_id: 0,
            focused_pin: None,
        }
    }

    fn capture(&mut self, ctx: &egui::Context) {
        if self.capture_gate.active().is_some() {
            self.capture_gate.cancel();
            self.capture_result = None;
            return;
        }
        if self.selection.is_some() || self.recording.blocks_input(ctx) {
            return;
        }
        let Some(_) = self.capture_gate.begin() else {
            return;
        };
        for pin in &self.pins {
            pin_view::forget_input(ctx, pin.id);
        }
        self.message = None;
        self.menu = None;
        self.unlock_hint = None;
        // Eframe's web backend reads the painted WebGL canvas. Keep the normal
        // desktop visible until that immutable frame returns as a screenshot.
        // Each request owns its receiver. Cancelling drops it, so an old callback
        // cannot supply pixels to a newer selection session.
        let (sender, receiver) = mpsc::sync_channel(1);
        self.capture_result = Some(receiver);
        let repaint = ctx.clone();
        ctx.request_screenshot(move |image| {
            if sender.try_send(image).is_ok() {
                repaint.request_repaint();
            }
        });
    }

    fn receive_capture(&mut self, ctx: &egui::Context) {
        let Some(request) = self.capture_gate.active() else {
            self.capture_result = None;
            return;
        };
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.capture_gate.cancel();
            self.capture_result = None;
            return;
        }
        let screenshot = self
            .capture_result
            .as_ref()
            .and_then(|result| result.try_recv().ok());
        let Some(source) = screenshot else {
            return;
        };
        self.capture_result = None;
        if !self.capture_gate.complete(request) {
            return;
        }
        let layers = ctx.memory(|memory| memory.layer_ids().collect::<Vec<_>>());
        self.window_rects
            .sort_by_key(|(layer, _)| layers.iter().position(|candidate| candidate == layer));
        let windows = self
            .window_rects
            .iter()
            .map(|(_, rect)| *rect)
            .collect::<Vec<_>>();
        self.selection = Some(FrozenCapture {
            selection: overlay::Selection {
                texture: ctx.load_texture("generated-desktop", source.clone(), Default::default()),
                pixels: source.size.map(|n| n as u32),
                pixel_step: [1, 1],
                windows: desktop::pixel_regions(self.desktop_bounds, source.size, &windows),
                press: None,
                start: None,
                // A browser simulation must not request native window activation.
                focus_requested: true,
            },
            pixels: source,
        });
    }

    fn pin_action(&mut self, action: pin_view::Action, ui: &egui::Ui, bounds: Rect) {
        match action {
            pin_view::Action::UnlockHint(id) => {
                if self.pins.iter().any(|pin| pin.id == id && pin.locked) {
                    let duration = snip2pin::ui::screen_hint::UNLOCK_DURATION;
                    self.unlock_hint =
                        Some((id, ui.input(|input| input.time) + duration.as_secs_f64()));
                    ui.ctx().request_repaint_after(duration);
                }
            }
            pin_view::Action::UnlockAll => {
                for pin in &mut self.pins {
                    if pin.locked {
                        pin_view::forget_input(ui.ctx(), pin.id);
                    }
                    pin.locked = false;
                }
                self.unlock_hint = None;
            }
            pin_view::Action::ClosePin(id) => {
                self.close_pin(id, ui.ctx());
            }
            pin_view::Action::ToggleLock(id) => {
                if let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id) {
                    pin_view::forget_input(ui.ctx(), id);
                    pin.locked = !pin.locked;
                    pin.gesture = None;
                }
                self.menu = None;
            }
            pin_view::Action::CopyPin(id) => {
                if self.pins.iter().any(|pin| pin.id == id) {
                    self.message = Some(Text::DemoCopied.into());
                }
                self.menu = None;
            }
            pin_view::Action::MenuPin(id) => {
                let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id) else {
                    return;
                };
                pin.gesture = None;
                pin_view::forget_input(ui.ctx(), id);
                let position = pin::menu_position(
                    ui.ctx().pointer_latest_pos().unwrap_or(bounds.min),
                    pin_view::MENU_SIZE,
                    &[],
                    bounds,
                    1.0,
                );
                self.menu = Some((id, position));
            }
            pin_view::Action::Move(id, steps) => {
                if let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id && !pin.locked)
                    && let Some(delta) = pin::movement_delta(
                        steps,
                        ui.ctx().native_pixels_per_point().unwrap_or(1.0),
                    )
                {
                    pin.image = pin.image.translate(delta / ui.ctx().zoom_factor());
                }
            }
            pin_view::Action::Zoom(id, commands) => {
                if let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id && !pin.locked)
                    && let Some(rect) = pin::zoom_rect(
                        egui::Rect::from_min_size(
                            pin.image.min * ui.ctx().zoom_factor(),
                            pin.image.size() * ui.ctx().zoom_factor(),
                        ),
                        pin.texture.size(),
                        ui.ctx().native_pixels_per_point().unwrap_or(1.0),
                        &commands,
                    )
                {
                    pin.image = Rect::from_min_size(
                        rect.min / ui.ctx().zoom_factor(),
                        rect.size() / ui.ctx().zoom_factor(),
                    );
                }
                self.menu = None;
            }
        }
    }

    fn close_pin(&mut self, id: u64, ctx: &egui::Context) {
        let Some(index) = self.pins.iter().position(|pin| pin.id == id && !pin.locked) else {
            return;
        };
        pin_view::forget(ctx, id);
        self.pins.remove(index);
        if self.focused_pin == Some(id) {
            self.focused_pin = None;
        }
        if self.menu.is_some_and(|(pin, _)| pin == id) {
            self.menu = None;
        }
    }

    fn record_key(&mut self, ctx: &egui::Context) {
        if let Some(binding) = self.recording.update(ctx) {
            self.preferences.binding = binding;
            self.recording.saved(Text::ShortcutConfirmed.into());
        }
    }

    fn update_icon(&mut self, ctx: &egui::Context) {
        let variant = self.preferences.icon_variant(ctx.system_theme());
        if self.applied_icon != variant
            && let Ok(pixels) = snip2pin::icon::render(variant, snip2pin::icon::WINDOW_SIZE)
        {
            self.icon.set(
                ColorImage::from_rgba_unmultiplied(
                    [pixels.width() as usize, pixels.height() as usize],
                    pixels.as_raw(),
                ),
                Default::default(),
            );
            self.applied_icon = variant;
        }
    }
}

impl Sandbox {
    fn show(&mut self, ui: &mut egui::Ui) {
        // A virtual window shares the root event stream. Preserve modal ownership
        // through this frame even when Escape just finished recording or a menu.
        let input_was_blocked = self.recording.blocks_input(ui.ctx()) || self.menu.is_some();
        let locale = self.preferences.language.resolve(self.system_locale);
        i18n::set_locale(ui.ctx(), locale);
        self.update_icon(ui.ctx());
        let bounds = ui.max_rect();
        // A frozen desktop belongs to one canvas layout. Resizing cancels the
        // selection rather than stretching stale window coordinates over a new scene.
        if bounds != self.desktop_bounds {
            self.capture_gate.cancel();
            self.selection = None;
        }
        self.receive_capture(ui.ctx());
        if !ui.input(|input| input.focused) {
            self.focused_pin = None;
        } else if self.menu.is_none() && ui.input(|input| input.pointer.any_pressed()) {
            self.focused_pin = ui.ctx().pointer_latest_pos().and_then(|point| {
                self.pins
                    .iter()
                    .rev()
                    .find(|pin| {
                        pin.image.contains(point)
                            && ui.ctx().layer_id_at(point)
                                == Some(egui::LayerId::new(
                                    egui::Order::Foreground,
                                    egui::Id::unique(("pin", pin.id)),
                                ))
                    })
                    .map(|pin| pin.id)
            });
        }
        if self.recording.active() {
            self.record_key(ui.ctx());
        } else if !self.recording.blocks_input(ui.ctx())
            && self.selection.is_none()
            && self.capture_gate.active().is_none()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| self.preferences.binding.capture_requested(&input.events))
        {
            self.capture(ui.ctx());
        }
        if let Some(capture) = &mut self.selection {
            match overlay::show(ui, &mut capture.selection, true) {
                Some(overlay::Action::Cancel) => self.selection = None,
                Some(overlay::Action::Select(rect)) => {
                    if let Some(pixels) = capture::crop(&capture.pixels, rect) {
                        self.next_id += 1;
                        let layout = pin::frame_geometry(
                            geometry::pin_placement(
                                rect,
                                capture.pixels.size.map(|n| n as u32),
                                bounds.min.into(),
                                bounds.size().into(),
                            ),
                            bounds,
                            1.0,
                        );
                        self.pins.push(Pin {
                            id: self.next_id,
                            name: String::new(),
                            texture: ui.ctx().load_texture(
                                format!("pin-{}", self.next_id),
                                pixels,
                                Default::default(),
                            ),
                            image: layout.image,
                            gesture: None,
                            locked: false,
                        });
                        self.message = Some(Text::DemoPinned.into());
                    }
                    self.selection = None;
                }
                _ => {}
            }
            return;
        }
        self.desktop_bounds = bounds;
        self.window_rects = desktop::show(ui, &self.wallpaper, &mut self.scene);
        let mut action = None;
        let message = self.message.as_ref().map(|message| message.render(locale));
        let pins = self
            .pins
            .iter()
            .rev()
            .map(|pin| launcher::PinEntry {
                id: pin.id,
                name: &pin.name,
                texture: &pin.texture,
                size: pin.texture.size(),
                display_size: Some(pin.image.size()),
                locked: pin.locked,
                selected: self.focused_pin == Some(pin.id),
            })
            .collect::<Vec<_>>();
        let home = egui::Window::new("Snip2Pin")
            .id(egui::Id::unique("home"))
            .title_bar(false)
            .default_pos(bounds.min + egui::vec2(36.0_f32.min(bounds.width() * 0.04), 66.0))
            .default_size(snip2pin::ui::style::LAUNCHER.window)
            .max_size((bounds.size() - egui::vec2(24.0, 90.0)).max(Vec2::splat(1.0)))
            .constrain_to(bounds.shrink2(egui::vec2(12.0, 42.0)))
            .resizable(true)
            .show(ui.ctx(), |ui| {
                action = launcher::show(
                    ui,
                    launcher::Model {
                        locale,
                        pins: &pins,
                        message: message.as_deref(),
                        global_shortcut: false,
                        preferences: self.preferences,
                        icon_texture: Some(&self.icon),
                        recording: &self.recording,
                        idle: self.capture_gate.active().is_none(),
                        capturing: self.capture_gate.active().is_some(),
                        demo: true,
                        startup: Some(self.startup),
                        permission_required: false,
                    },
                );
            });
        if let Some(home) = home {
            self.window_rects
                .push((home.response.layer_id, home.response.rect));
        }
        match action {
            Some(launcher::Action::RenamePin(id, name)) => {
                if let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id) {
                    pin.name = name;
                }
            }
            Some(launcher::Action::Capture) => self.capture(ui.ctx()),
            Some(launcher::Action::ClosePins) => {
                self.menu = None;
                let ids = self.pins.iter().map(|pin| pin.id).collect::<Vec<_>>();
                for id in ids {
                    self.close_pin(id, ui.ctx());
                }
            }
            Some(launcher::Action::Pin(action)) => self.pin_action(action, ui, bounds),
            Some(launcher::Action::SelectPin(id)) => {
                if self.pins.iter().any(|pin| pin.id == id) {
                    self.focused_pin = Some(id);
                    ui.ctx().move_to_top(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::unique(("pin", id)),
                    ));
                }
            }
            Some(launcher::Action::RecordShortcut) => {
                self.recording.toggle(ui.ctx());
            }
            Some(launcher::Action::Appearance(theme, icon)) => {
                self.preferences.theme = theme;
                self.preferences.icon = icon;
                ui.ctx().set_theme(theme.preference());
                self.update_icon(ui.ctx());
            }
            Some(launcher::Action::Startup(enabled)) => self.startup = enabled,
            Some(launcher::Action::Language(language)) => self.preferences.language = language,
            _ => {}
        }
        if ui.ctx().text_edit_focused() {
            self.focused_pin = None;
        }
        if self.menu.is_some()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.menu = None;
        }
        let events = RefCell::new(Vec::new());
        let pins_interactive =
            !input_was_blocked && !self.recording.blocks_input(ui.ctx()) && self.menu.is_none();
        for pin in &mut self.pins {
            let pointer = ui.ctx().pointer_latest_pos();
            if let Some(gesture) = &pin.gesture
                && let Some(pointer) = pointer
            {
                pin.image = gesture.image_at(pointer, bounds.size(), ui.ctx().pixels_per_point());
            }
            if !ui.input(|input| input.pointer.primary_down()) {
                pin.gesture = None;
            }
            self.window_rects.push((
                egui::LayerId::new(egui::Order::Foreground, egui::Id::unique(("pin", pin.id))),
                pin.image,
            ));
            let mut requested = None;
            egui::Area::new(egui::Id::unique(("pin", pin.id)))
                .order(egui::Order::Foreground)
                .movable(false)
                .constrain(false)
                .fixed_pos(pin.image.min)
                .show(ui.ctx(), |ui| {
                    if !pins_interactive {
                        ui.disable();
                    }
                    ui.set_min_size(pin.image.size());
                    ui.set_max_size(pin.image.size());
                    let controls = pin::toolbar_geometry(ui.max_rect(), &[bounds], bounds, 1.0);
                    pin_view::show(
                        ui,
                        pin.id,
                        &pin.texture,
                        pin_view::Interaction {
                            native_edges: false,
                            locked: pin.locked,
                            toolbar: controls,
                            keyboard_focus: Some(self.focused_pin == Some(pin.id)),
                            space: pin_view::InputSpace {
                                origin: Some(ui.max_rect().min),
                                scale: ui.ctx().pixels_per_point(),
                            },
                        },
                        &|action| {
                            events.borrow_mut().push(action);
                        },
                        |command| requested = Some(command),
                    );
                });
            if let Some(command) = requested
                && let Some(pointer) = ui.input(|input| input.pointer.press_origin())
            {
                let direction = match command {
                    ViewportCommand::StartDrag => Some(None),
                    ViewportCommand::BeginResize(direction) => Some(Some(direction)),
                    _ => None,
                };
                if let Some(direction) = direction {
                    pin.gesture = Some(Gesture {
                        origin: pointer,
                        image: pin.image,
                        direction,
                    });
                }
            }
        }
        for event in events.into_inner() {
            self.pin_action(event, ui, bounds);
        }
        if let Some((id, until)) = self.unlock_hint {
            if ui.input(|input| input.time) < until
                && self.pins.iter().any(|pin| pin.id == id && pin.locked)
            {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(
                        until - ui.input(|input| input.time),
                    ));
                snip2pin::ui::screen_hint::show(
                    ui,
                    bounds,
                    &snip2pin::ui::screen_hint::unlock_text(ui.ctx()),
                );
            } else {
                self.unlock_hint = None;
            }
        }
        if let Some((id, position)) = self.menu {
            let menu_rect = Rect::from_min_size(position, pin_view::MENU_SIZE);
            if !self.pins.iter().any(|pin| pin.id == id)
                || ui.input(|input| {
                    input.key_pressed(egui::Key::Escape)
                        || (input.pointer.any_pressed()
                            && input
                                .pointer
                                .interact_pos()
                                .is_some_and(|point| !menu_rect.contains(point)))
                })
            {
                self.menu = None;
            } else {
                let action = RefCell::new(None);
                egui::Area::new(egui::Id::unique("pin-menu"))
                    .order(egui::Order::Foreground)
                    .movable(false)
                    .constrain(false)
                    .fixed_pos(position)
                    .show(ui.ctx(), |ui| {
                        ui.set_min_size(pin_view::MENU_SIZE);
                        ui.set_max_size(pin_view::MENU_SIZE);
                        ui.painter()
                            .rect_filled(ui.max_rect(), 4, ui.visuals().window_fill);
                        let locked = self.pins.iter().any(|pin| pin.id == id && pin.locked);
                        pin_view::menu(ui, id, locked, &|event| *action.borrow_mut() = Some(event));
                    });
                if let Some(action) = action.into_inner() {
                    self.menu = None;
                    self.pin_action(action, ui, bounds);
                }
            }
        }
    }
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(
    font: Vec<u8>,
    wallpaper: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<(), JsValue> {
    let size = [width as usize, height as usize];
    if size.contains(&0)
        || size[0]
            .checked_mul(size[1])
            .and_then(|pixels| pixels.checked_mul(4))
            != Some(wallpaper.len())
    {
        return Err(JsValue::from_str("invalid preview wallpaper"));
    }
    let wallpaper = ColorImage::from_rgba_unmultiplied(size, &wallpaper);
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or_else(|| JsValue::from_str("canvas unavailable"))?
        .dyn_into()?;
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |cc| Ok(Box::new(Sandbox::new(&cc.egui_ctx, font, wallpaper)))),
        )
        .await
}

#[cfg(test)]
mod tests;
