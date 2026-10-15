use std::{cell::RefCell, sync::Arc};

use eframe::egui::{self, ColorImage, Pos2, Rect, TextureHandle, Vec2};
use snip2pin::{
    capture,
    geometry::Placement,
    pin::{self, Preparation},
    settings::{Binding, Preferences},
    ui::{launcher, overlay, pin as pin_view},
};
use wasm_bindgen::{JsCast, prelude::*};

struct Pin {
    id: u64,
    texture: TextureHandle,
    prepared: Preparation,
    original: Vec2,
}

struct Sandbox {
    preferences: Preferences,
    recording: bool,
    startup: bool,
    message: Option<String>,
    source: Arc<ColorImage>,
    texture: TextureHandle,
    icon: TextureHandle,
    selection: Option<overlay::Selection>,
    pins: Vec<Pin>,
    next_id: u64,
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
            next_id: 0,
        }
    }

    fn capture(&mut self) {
        self.message = None;
        self.selection = Some(overlay::Selection {
            texture: self.texture.clone(),
            pixels: self.source.size.map(|n| n as u32),
            start: None,
            focus_requested: false,
        });
    }
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
                        let frame = pin::frame_geometry(
                            Placement {
                                position: image.min.into(),
                                size: image.size().into(),
                            },
                            bounds,
                            1.0,
                            1.0,
                        );
                        let mut prepared = Preparation::default();
                        prepared.commit(frame);
                        self.pins.push(Pin {
                            id: self.next_id,
                            texture: ui.ctx().load_texture(
                                format!("pin-{}", self.next_id),
                                pixels,
                                Default::default(),
                            ),
                            prepared,
                            original: image.size(),
                        });
                        self.message = Some("已复制并置顶（沙箱仅模拟复制）。".into());
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
            "本地 UI 沙箱 · 生成图片 · 不访问桌面 / 剪贴板 / 系统设置",
            egui::FontId::proportional(13.0),
            ui.visuals().weak_text_color(),
        );
        let mut action = None;
        egui::Window::new("Snip2Pin")
            .id(egui::Id::new("home"))
            .title_bar(false)
            .fixed_pos(egui::pos2(24.0, 24.0))
            .fixed_size(snip2pin::ui::style::LAUNCHER.window)
            .show(ui.ctx(), |ui| {
                action = launcher::show(
                    ui,
                    launcher::Model {
                        pin_count: self.pins.len(),
                        message: self.message.as_deref(),
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
            Some(launcher::Action::ClosePins) => self.pins.clear(),
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
            _ => {}
        }
        let events = RefCell::new(Vec::new());
        for pin in &mut self.pins {
            let image = pin
                .prepared
                .image(1.0)
                .unwrap_or(Rect::from_min_size(Pos2::ZERO, pin.original));
            let mut frame = pin::frame_geometry(
                Placement {
                    position: image.min.into(),
                    size: image.size().into(),
                },
                bounds,
                1.0,
                1.0,
            );
            frame.image = frame.image.translate(frame.window.min.to_vec2());
            let window = frame.window;
            frame.window = bounds;
            egui::Area::new(egui::Id::new(("pin", pin.id)))
                .order(egui::Order::Foreground)
                .movable(false)
                .fixed_pos(window.min)
                .show(ui.ctx(), |ui| {
                    ui.set_min_size(window.size());
                    let pointer = ui.ctx().pointer_latest_pos().map(Into::into);
                    pin_view::show(
                        ui,
                        pin_view::Model {
                            id: pin.id,
                            texture: &pin.texture,
                            frame,
                            events: &|action| events.borrow_mut().push(action),
                            monitors: &[bounds],
                            coordinate_ratio: 1.0,
                            minimum_size: Vec2::splat(1.0),
                            surface: pin_view::Surface::Embedded,
                        },
                        &mut pin.prepared,
                        || pointer,
                    );
                });
        }
        for event in events.into_inner() {
            match event {
                pin_view::Action::ClosePin(id) => self.pins.retain(|pin| pin.id != id),
                pin_view::Action::CopyPin(_) => {
                    self.message = Some("已复制（沙箱仅模拟，不写系统剪贴板）。".into())
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
