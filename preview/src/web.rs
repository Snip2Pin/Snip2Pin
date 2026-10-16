use std::{cell::RefCell, sync::Arc};

use eframe::egui::{
    self, ColorImage, Pos2, Rect, ResizeDirection, TextureHandle, Vec2, ViewportCommand,
};
use snip2pin::{
    capture,
    geometry::Placement,
    i18n::{self, Locale, Message, Text},
    pin,
    settings::{Binding, Preferences},
    ui::{launcher, overlay, pin as pin_view},
};
use wasm_bindgen::{JsCast, prelude::*};

struct Pin {
    id: u64,
    texture: TextureHandle,
    image: Rect,
    gesture: Option<Gesture>,
    locked: bool,
    highlight_until: Option<f64>,
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
    recording: bool,
    startup: bool,
    message: Option<Message>,
    system_locale: Locale,
    source: Arc<ColorImage>,
    texture: TextureHandle,
    icon: TextureHandle,
    selection: Option<overlay::Selection>,
    pins: Vec<Pin>,
    menu: Option<(u64, Pos2)>,
    next_id: u64,
    selected_pin: Option<u64>,
}

impl Sandbox {
    fn new(cc: &eframe::CreationContext<'_>, font: Vec<u8>) -> Self {
        if !font.is_empty() {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "sandbox-cjk".into(),
                Arc::new(egui::FontData::from_owned(font)),
            );
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.insert(0, "sandbox-cjk".into());
            }
            cc.egui_ctx.set_fonts(fonts);
        }
        let source = capture::demo().pixels;
        Self {
            preferences: Preferences::default(),
            recording: false,
            startup: false,
            message: None,
            system_locale: Locale::system(),
            texture: cc.egui_ctx.load_texture(
                "generated-desktop",
                source.clone(),
                Default::default(),
            ),
            icon: cc.egui_ctx.load_texture(
                "brand",
                snip2pin::icon::render(snip2pin::icon::Variant::Line, 64)
                    .map(|pixels| ColorImage::from_rgba_unmultiplied([64, 64], pixels.as_raw()))
                    .unwrap_or_else(|_| ColorImage::filled([1, 1], egui::Color32::TRANSPARENT)),
                Default::default(),
            ),
            source,
            selection: None,
            pins: Vec::new(),
            menu: None,
            next_id: 0,
            selected_pin: None,
        }
    }

    fn capture(&mut self) {
        self.message = None;
        self.menu = None;
        self.selection = Some(overlay::Selection {
            texture: self.texture.clone(),
            pixels: self.source.size.map(|n| n as u32),
            pixel_step: [1, 1],
            start: None,
            focus_requested: false,
        });
    }

    fn pin_action(&mut self, action: pin_view::Action, ui: &egui::Ui, bounds: Rect) {
        match action {
            pin_view::Action::ClosePin(id) => {
                self.pins.retain(|pin| pin.id != id || pin.locked);
                if !self
                    .pins
                    .iter()
                    .any(|pin| Some(pin.id) == self.selected_pin)
                {
                    self.selected_pin = None;
                }
                self.menu = None;
            }
            pin_view::Action::ToggleLock(id) => {
                if let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id) {
                    pin.locked = !pin.locked;
                    pin.gesture = None;
                }
                self.menu = None;
            }
            pin_view::Action::CopyPin(_) => {
                self.message = Some(Text::DemoCopied.into());
                self.menu = None;
            }
            pin_view::Action::MenuPin(id) => {
                let position = ui.ctx().pointer_latest_pos().unwrap_or(bounds.min).clamp(
                    bounds.min,
                    (bounds.max - pin_view::MENU_SIZE).max(bounds.min),
                );
                self.menu = Some((id, position));
            }
        }
    }
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let locale = self.preferences.language.resolve(self.system_locale);
        i18n::set_locale(ui.ctx(), locale);
        let bounds = ui.max_rect();
        if self.recording {
            for event in ui.input(|input| input.events.clone()) {
                if let egui::Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    repeat: false,
                    ..
                } = event
                {
                    if key == egui::Key::Escape {
                        self.recording = false;
                        break;
                    }
                    match Binding::from_key(key, modifiers) {
                        Ok(binding) => {
                            self.preferences.binding = binding;
                            self.recording = false;
                            self.message = None;
                        }
                        Err(error) => self.message = Some(error),
                    }
                    break;
                }
            }
        } else if self.selection.is_none()
            && ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| self.preferences.binding.matches(event))
            })
        {
            self.capture();
        }
        if let Some(selection) = &mut self.selection {
            match overlay::show(ui, selection, true) {
                Some(overlay::Action::Cancel) => self.selection = None,
                Some(overlay::Action::Select(rect)) => {
                    if let Some(pixels) = capture::crop(&self.source, rect) {
                        self.next_id += 1;
                        let image = Rect::from_min_size(
                            bounds.min
                                + egui::vec2(
                                    rect.x as f32 / self.source.size[0] as f32 * bounds.width(),
                                    rect.y as f32 / self.source.size[1] as f32 * bounds.height(),
                                ),
                            egui::vec2(
                                rect.width as f32 / self.source.size[0] as f32 * bounds.width(),
                                rect.height as f32 / self.source.size[1] as f32 * bounds.height(),
                            ),
                        );
                        let layout = pin::frame_geometry(
                            Placement {
                                position: image.min.into(),
                                size: image.size().into(),
                            },
                            bounds,
                            1.0,
                        );
                        self.pins.push(Pin {
                            id: self.next_id,
                            texture: ui.ctx().load_texture(
                                format!("pin-{}", self.next_id),
                                pixels,
                                Default::default(),
                            ),
                            image: layout.image,
                            gesture: None,
                            locked: false,
                            highlight_until: None,
                        });
                        self.message = Some(Text::DemoPinned.into());
                    }
                    self.selection = None;
                }
                _ => {}
            }
            return;
        }
        ui.painter()
            .rect_filled(bounds, 0, ui.visuals().extreme_bg_color);
        ui.painter().text(
            bounds.right_bottom() - Vec2::splat(20.0),
            egui::Align2::RIGHT_BOTTOM,
            locale.text(Text::Sandbox),
            egui::FontId::proportional(13.0),
            ui.visuals().weak_text_color(),
        );
        let mut action = None;
        let message = self.message.as_ref().map(|message| message.render(locale));
        let pins = self
            .pins
            .iter()
            .rev()
            .map(|pin| launcher::PinEntry {
                id: pin.id,
                texture: &pin.texture,
                size: pin.texture.size(),
                locked: pin.locked,
                selected: self.selected_pin == Some(pin.id),
            })
            .collect::<Vec<_>>();
        egui::Window::new("Snip2Pin")
            .id(egui::Id::new("home"))
            .title_bar(false)
            .fixed_pos(egui::pos2(24.0, 24.0))
            .fixed_size(snip2pin::ui::style::LAUNCHER.window)
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
                        recording: self.recording,
                        idle: true,
                        capturing: false,
                        demo: true,
                        startup: Some(self.startup),
                        permission_required: false,
                    },
                );
            });
        match action {
            Some(launcher::Action::Capture) => self.capture(),
            Some(launcher::Action::ClosePins) => self.pins.retain(|pin| pin.locked),
            Some(launcher::Action::Pin(action)) => self.pin_action(action, ui, bounds),
            Some(launcher::Action::SelectPin(id)) => {
                if let Some(index) = self.pins.iter().position(|pin| pin.id == id) {
                    let mut pin = self.pins.remove(index);
                    pin.highlight_until = Some(
                        ui.input(|input| input.time)
                            + snip2pin::ui::style::PIN.highlight_duration.as_secs_f64(),
                    );
                    self.pins.push(pin);
                    self.selected_pin = Some(id);
                    ui.ctx().move_to_top(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new(("pin", id)),
                    ));
                }
            }
            Some(launcher::Action::RecordShortcut) => self.recording = !self.recording,
            Some(launcher::Action::Appearance(theme, icon)) => {
                self.preferences.theme = theme;
                self.preferences.icon = icon;
                ui.ctx().set_theme(theme.preference());
                if let Ok(pixels) = snip2pin::icon::render(
                    self.preferences.icon_variant(ui.ctx().system_theme()),
                    64,
                ) {
                    self.icon.set(
                        ColorImage::from_rgba_unmultiplied([64, 64], pixels.as_raw()),
                        Default::default(),
                    );
                }
            }
            Some(launcher::Action::Startup(enabled)) => self.startup = enabled,
            Some(launcher::Action::Language(language)) => self.preferences.language = language,
            _ => {}
        }
        if self.menu.is_some()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.menu = None;
        }
        let events = RefCell::new(Vec::new());
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
            let mut requested = None;
            egui::Area::new(egui::Id::new(("pin", pin.id)))
                .order(egui::Order::Foreground)
                .movable(false)
                .constrain(false)
                .fixed_pos(pin.image.min)
                .show(ui.ctx(), |ui| {
                    ui.set_min_size(pin.image.size());
                    ui.set_max_size(pin.image.size());
                    let controls = pin::toolbar_geometry(ui.max_rect(), &[bounds], bounds, 1.0);
                    let hovered = pointer.is_some_and(|point| ui.max_rect().contains(point));
                    pin_view::image_with_host(
                        ui,
                        pin.id,
                        &pin.texture,
                        pin_view::Interaction {
                            native_edges: false,
                            locked: pin.locked,
                            excluded: controls.map(|toolbar| toolbar.bounds),
                        },
                        &|action| events.borrow_mut().push(action),
                        |command| requested = Some(command),
                    );
                    if let Some(until) = pin.highlight_until {
                        let remaining = until - ui.input(|input| input.time);
                        if remaining > 0.0 {
                            let style = &snip2pin::ui::style::PIN;
                            ui.painter().rect_stroke(
                                ui.max_rect(),
                                0,
                                egui::Stroke::new(style.highlight_stroke, style.highlight),
                                egui::StrokeKind::Inside,
                            );
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_secs_f64(
                                    remaining,
                                ));
                        }
                    }
                    if hovered && let Some(row) = controls {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(row.bounds), |ui| {
                            pin_view::controls(ui, pin.id, pin.locked, row, &|action| {
                                events.borrow_mut().push(action)
                            });
                        });
                    }
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
                egui::Area::new(egui::Id::new("pin-menu"))
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

#[wasm_bindgen]
pub async fn start(font: Vec<u8>) -> Result<(), JsValue> {
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
            Box::new(move |cc| Ok(Box::new(Sandbox::new(cc, font)))),
        )
        .await
}
