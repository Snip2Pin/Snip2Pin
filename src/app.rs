use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

use eframe::egui::{self, ColorImage, TextureHandle, ViewportCommand, ViewportId};
use snip2pin::{
    capture::{self, Display, Snapshot},
    geometry::{self, Placement},
    icon::{self, Variant},
    pin::{Coordinates, Preparation as PinPreparation, Prepared},
    session::CaptureGate,
    settings::{self, Binding, Preferences},
};

use crate::{
    platform::{self, Shortcut},
    ui::{self, launcher, overlay},
};

const SHORTCUT_SUPPRESSION: Duration = Duration::from_millis(200);
const OVERLAY_SIZE_TOLERANCE: f32 = 0.5;

pub enum Event {
    Capture,
    Ready(u64, Result<Vec<Snapshot>, String>, Option<[f32; 2]>),
    Selection(u64, u32, overlay::Action),
    PinReady(u64, u64),
    CopyPin(u64),
    ClosePin(u64),
}

enum Phase {
    Idle,
    Capturing,
    Selecting {
        id: u64,
        snapshots: Vec<Snapshot>,
        pending_pin: Option<u64>,
    },
}

struct Layer {
    display: Display,
    selection: Option<Arc<Mutex<overlay::Selection>>>,
    positioned: Arc<AtomicBool>,
}

struct Pin {
    texture: TextureHandle,
    pixels: Arc<ColorImage>,
    placement: Placement,
    display_scale: f32,
    screen: egui::Rect,
    screen_areas: Arc<Vec<egui::Rect>>,
    session: u64,
    prepared: Arc<Mutex<PinPreparation>>,
}

pub struct Snip2Pin {
    events: Receiver<Event>,
    sender: Sender<Event>,
    phase: Phase,
    gate: CaptureGate,
    layers: BTreeMap<u32, Layer>,
    active_display: Option<u32>,
    pins: BTreeMap<u64, Pin>,
    next_pin: u64,
    message: Option<String>,
    shortcut: Option<Shortcut>,
    preferences: Preferences,
    applied_icon: Option<Variant>,
    icon_data: Option<Arc<egui::IconData>>,
    icon_texture: Option<TextureHandle>,
    config_path: Option<PathBuf>,
    recording: bool,
    suppress_shortcut_until: Instant,
    demo: bool,
}

impl Snip2Pin {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        platform::install_fonts(&cc.egui_ctx);
        let demo = std::env::args().any(|argument| argument == "--demo");
        let (sender, events) = mpsc::channel();
        let config_path = (!demo).then(settings::path).flatten();
        let loaded = config_path
            .as_deref()
            .map(Preferences::load)
            .unwrap_or(Ok(Preferences::default()));
        let mut message = loaded.as_ref().err().cloned();
        let preferences = loaded.unwrap_or_default();
        cc.egui_ctx
            .options_mut(|options| options.fallback_theme = egui::Theme::Light);
        cc.egui_ctx.set_theme(preferences.theme.preference());
        let shortcut = if demo {
            None
        } else {
            let ctx = cc.egui_ctx.clone();
            let hotkey_sender = sender.clone();
            match Shortcut::register(preferences.binding, move || {
                let _ = hotkey_sender.send(Event::Capture);
                ctx.request_repaint_of(ViewportId::ROOT);
            }) {
                Ok(shortcut) => Some(shortcut),
                Err(error) => {
                    message = Some(format!("全局快捷键不可用：{error}"));
                    None
                }
            }
        };
        let displays = if demo {
            Ok(vec![capture::demo().display])
        } else {
            capture::displays()
        };
        if let Err(error) = &displays {
            message = Some(error.clone());
        }
        let layers = displays
            .unwrap_or_default()
            .into_iter()
            .map(|display| {
                (
                    display.id,
                    Layer {
                        display,
                        selection: None,
                        positioned: Arc::new(AtomicBool::new(false)),
                    },
                )
            })
            .collect();
        Self {
            events,
            sender,
            phase: Phase::Idle,
            gate: CaptureGate::default(),
            layers,
            active_display: None,
            pins: BTreeMap::new(),
            next_pin: 0,
            message,
            shortcut,
            preferences,
            applied_icon: None,
            icon_data: None,
            icon_texture: None,
            config_path,
            recording: false,
            suppress_shortcut_until: Instant::now(),
            demo,
        }
    }

    fn begin_capture(&mut self, ctx: &egui::Context) {
        if !matches!(self.phase, Phase::Idle) || self.recording {
            return;
        }
        let Some(id) = self.gate.begin() else {
            return;
        };
        self.message = None;
        self.phase = Phase::Capturing;
        // Preserve the visible desktop while capturing. No hiding, animation,
        // fullscreen transition or artificial delay is needed.
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        let demo = self.demo;
        let demo_origin = ctx.input(|input| {
            input
                .viewport()
                .outer_rect
                .map(|rect| [rect.min.x, rect.min.y])
        });
        std::thread::spawn(move || {
            let result = if demo {
                let mut snapshot = capture::demo();
                if let Some(origin) = demo_origin {
                    snapshot.display.origin = origin;
                }
                Ok(vec![snapshot])
            } else {
                capture::desktop()
            };
            let pointer = if !demo && result.is_ok() {
                platform::pointer_position()
            } else {
                None
            };
            let _ = sender.send(Event::Ready(id, result, pointer));
            ctx.request_repaint_of(ViewportId::ROOT);
        });
    }

    fn restore(&mut self, ctx: &egui::Context) {
        let was_selecting = matches!(self.phase, Phase::Selecting { .. });
        for (&display_id, layer) in &mut self.layers {
            layer.selection = None;
            ctx.request_repaint_of(layer_id(display_id));
        }
        self.phase = Phase::Idle;
        self.active_display = None;
        if was_selecting {
            platform::finish_capture(ctx);
        }
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn accept_capture(
        &mut self,
        id: u64,
        result: Result<Vec<Snapshot>, String>,
        pointer: Option<[f32; 2]>,
        ctx: &egui::Context,
    ) {
        if !self.gate.complete(id) {
            return;
        }
        match result {
            Ok(snapshots) if !snapshots.is_empty() => {
                self.active_display = pointer
                    .and_then(|point| snapshots.iter().find(|shot| shot.display.contains(point)))
                    .or_else(|| snapshots.first())
                    .map(|shot| shot.display.id);
                // Refresh topology on every capture, including display hot-plug.
                self.layers = snapshots
                    .iter()
                    .map(|snapshot| {
                        let display = snapshot.display.clone();
                        let texture =
                            texture(ctx, &format!("selection-{}", display.id), &snapshot.pixels);
                        let selection = overlay::Selection {
                            texture,
                            pixels: [snapshot.pixels.width(), snapshot.pixels.height()],
                            start: None,
                            focus_requested: false,
                        };
                        (
                            display.id,
                            Layer {
                                display,
                                selection: Some(Arc::new(Mutex::new(selection))),
                                positioned: Arc::new(AtomicBool::new(false)),
                            },
                        )
                    })
                    .collect();
                self.phase = Phase::Selecting {
                    id,
                    snapshots,
                    pending_pin: None,
                };
                for &display_id in self.layers.keys() {
                    ctx.request_repaint_of(layer_id(display_id));
                }
                ctx.request_repaint_of(ViewportId::ROOT);
            }
            result => {
                self.message = Some(result.err().unwrap_or_else(|| "没有可用屏幕。".into()));
                self.restore(ctx);
            }
        }
    }

    fn selection_action(
        &mut self,
        id: u64,
        display_id: u32,
        action: overlay::Action,
        ctx: &egui::Context,
    ) {
        let Phase::Selecting {
            id: current,
            snapshots,
            pending_pin,
        } = &mut self.phase
        else {
            return;
        };
        if id != *current {
            return;
        }
        if matches!(action, overlay::Action::Cancel) {
            if let Some(pin) = *pending_pin {
                self.pins.remove(&pin);
            }
            self.restore(ctx);
            return;
        }
        if pending_pin.is_some() {
            return;
        }
        if matches!(action, overlay::Action::Activate) {
            if self.layers.contains_key(&display_id) {
                self.active_display = Some(display_id);
                for &display in self.layers.keys() {
                    ctx.request_repaint_of(layer_id(display));
                }
            }
            return;
        }
        if let overlay::Action::Select(rect) = action
            && let Some(snapshot) = snapshots.iter().find(|shot| shot.display.id == display_id)
            && let Some(pixels) = capture::crop(&snapshot.pixels, rect)
        {
            self.next_pin += 1;
            let placement = geometry::pin_placement(
                rect,
                [snapshot.pixels.width(), snapshot.pixels.height()],
                snapshot.display.position(),
                snapshot.display.logical_size(),
            );
            let pixels = Arc::new(color_image(&pixels));
            let texture = ctx.load_texture(
                format!("pin-{}", self.next_pin),
                pixels.clone(),
                egui::TextureOptions::LINEAR,
            );
            ctx.copy_image((*pixels).clone());
            let screen = if self.demo {
                egui::Rect::from_min_size(
                    snapshot.display.position().into(),
                    snapshot.display.logical_size().into(),
                )
            } else {
                platform::pin_work_area(&snapshot.display)
            };
            let screen_areas = Arc::new(
                self.layers
                    .values()
                    .map(|layer| {
                        let area = egui::Rect::from_min_size(
                            layer.display.position().into(),
                            layer.display.logical_size().into(),
                        );
                        // Normalize mixed-DPI display bounds to the capture's
                        // coordinate scale before the viewport applies its DPI.
                        let factor = if cfg!(target_os = "macos") {
                            1.0
                        } else {
                            layer.display.scale / snapshot.display.scale
                        };
                        egui::Rect::from_min_max(area.min * factor, area.max * factor)
                    })
                    .collect(),
            );
            self.pins.insert(
                self.next_pin,
                Pin {
                    texture,
                    pixels,
                    placement,
                    display_scale: snapshot.display.scale,
                    screen,
                    screen_areas,
                    session: id,
                    prepared: Arc::new(Mutex::new(PinPreparation::default())),
                },
            );
            // Keep the selection image until the pin has painted its first frame.
            *pending_pin = Some(self.next_pin);
            ctx.request_repaint_of(ViewportId::ROOT);
        } else {
            self.restore(ctx);
        }
    }

    fn copy_pin(&self, id: u64, ctx: &egui::Context) {
        if let Some(pin) = self.pins.get(&id) {
            ctx.copy_image((*pin.pixels).clone());
        }
    }

    fn close_pin(&mut self, id: u64, ctx: &egui::Context) {
        self.pins.remove(&id);
        if matches!(&self.phase, Phase::Selecting { pending_pin: Some(pending), .. } if *pending == id)
        {
            self.restore(ctx);
        }
    }

    fn change_binding(&mut self, binding: Binding, ctx: &egui::Context) {
        self.suppress_shortcut_until = Instant::now() + SHORTCUT_SUPPRESSION;
        if self.shortcut.is_none() && !self.demo && platform::global_shortcuts_supported() {
            let sender = self.sender.clone();
            let ctx = ctx.clone();
            match Shortcut::register(binding, move || {
                let _ = sender.send(Event::Capture);
                ctx.request_repaint_of(ViewportId::ROOT);
            }) {
                Ok(shortcut) => self.shortcut = Some(shortcut),
                Err(error) => {
                    self.message = Some(format!("快捷键不可用，请换一个组合：{error}"));
                    return;
                }
            }
        }
        if let Some(shortcut) = &mut self.shortcut
            && let Err(error) = shortcut.replace(binding)
        {
            self.message = Some(error);
            return;
        }
        let preferences = Preferences {
            binding,
            ..self.preferences
        };
        if let Some(path) = &self.config_path
            && let Err(error) = preferences.save(path)
        {
            let rollback = self
                .shortcut
                .as_mut()
                .map(|shortcut| shortcut.replace(self.preferences.binding));
            if matches!(rollback, Some(Err(_))) {
                // Show the actual registered binding if rollback fails.
                self.preferences.binding = binding;
            }
            self.message = Some(error);
            return;
        }
        self.preferences = preferences;
        self.message = Some(
            if self.shortcut.is_some() {
                "快捷键已保存。"
            } else {
                "快捷键已保存；当前桌面仅支持窗口内快捷键。"
            }
            .into(),
        );
    }

    fn change_appearance(
        &mut self,
        theme: settings::Theme,
        icon: settings::Icon,
        ctx: &egui::Context,
    ) {
        ctx.request_repaint_of(ViewportId::ROOT);
        let preferences = Preferences {
            theme,
            icon,
            ..self.preferences
        };
        if let Some(path) = &self.config_path
            && let Err(error) = preferences.save(path)
        {
            self.message = Some(error);
            return;
        }
        self.preferences = preferences;
        ctx.set_theme(theme.preference());
        self.message = Some(
            if self.demo {
                "演示外观已更新，本次有效。"
            } else {
                "外观设置已保存。"
            }
            .into(),
        );
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn update_icon(&mut self, ctx: &egui::Context) {
        let variant = self.preferences.icon_variant(ctx.system_theme());
        if self.applied_icon == Some(variant) {
            return;
        }
        self.applied_icon = Some(variant);
        match icon::render(variant, icon::WINDOW_SIZE) {
            Ok(pixels) => {
                self.icon_texture = Some(texture(ctx, "application-icon", &pixels));
                let data = Arc::new(egui::IconData {
                    width: pixels.width(),
                    height: pixels.height(),
                    rgba: pixels.into_raw(),
                });
                self.icon_data = Some(data.clone());
                if let Err(error) = platform::set_app_icon(ctx, variant, data) {
                    self.message = Some(error);
                }
            }
            Err(error) => {
                self.message = Some(error);
            }
        }
    }

    fn window_icon(&self, builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
        match &self.icon_data {
            Some(data) => builder.with_icon(data.clone()),
            None => builder,
        }
    }

    fn record_key(&mut self, ui: &egui::Ui) {
        let events = ui.input(|input| input.events.clone());
        for event in events {
            if let egui::Event::Key {
                key,
                physical_key,
                modifiers,
                pressed: true,
                repeat: false,
                ..
            } = event
            {
                if key == egui::Key::Escape {
                    self.recording = false;
                    return;
                }
                match Binding::from_key(physical_key.unwrap_or(key), modifiers) {
                    Ok(binding) => {
                        self.recording = false;
                        self.change_binding(binding, ui.ctx());
                    }
                    Err(error) => {
                        self.message = Some(error);
                    }
                }
                return;
            }
        }
    }

    fn show_layers(&self, ctx: &egui::Context) {
        let session = match &self.phase {
            Phase::Selecting { id, .. } => Some(*id),
            _ => None,
        };
        let demo = self.demo;
        for (&display_id, layer) in &self.layers {
            let selection = layer.selection.clone();
            let positioned = layer.positioned.clone();
            let events = self.sender.clone();
            let active = selection.is_some();
            let pointer_display = self.active_display == Some(display_id);
            let display = layer.display.clone();
            let position = display.position();
            let size = display.logical_size();
            ctx.show_viewport_deferred(
                layer_id(display_id),
                self.window_icon(egui::ViewportBuilder::default())
                    .with_title("Snip2Pin · 框选")
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_has_shadow(false)
                    .with_position(position)
                    .with_inner_size(size)
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_active(false)
                    .with_taskbar(false)
                    .with_mouse_passthrough(!active),
                move |ui, _| {
                    platform::prepare_floating_windows();
                    if cfg!(target_os = "macos") && !demo {
                        // Native bounds can change after this frame's input was
                        // collected. Wait for the resize event instead of squeezing
                        // the whole desktop into a menu-bar-constrained viewport.
                        let expected = egui::vec2(size[0], size[1]) / ui.ctx().zoom_factor();
                        let actual = ui.max_rect().size();
                        if (actual.x - expected.x).abs() > OVERLAY_SIZE_TOLERANCE
                            || (actual.y - expected.y).abs() > OVERLAY_SIZE_TOLERANCE
                        {
                            ui.ctx().request_repaint();
                            return;
                        }
                    }
                    // Correct initial placement after the native window resolves
                    // its actual monitor/DPI (especially mixed-DPI Windows).
                    if !cfg!(target_os = "macos") && !positioned.swap(true, Ordering::AcqRel) {
                        let scale = ui.ctx().pixels_per_point();
                        let target =
                            egui::pos2(display.origin[0] / scale, display.origin[1] / scale);
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::OuterPosition(target));
                    }
                    if let (Some(selection), Some(id)) = (&selection, session)
                        && let Ok(mut selection) = selection.lock()
                        && let Some(action) = overlay::show(ui, &mut selection, pointer_display)
                    {
                        let _ = events.send(Event::Selection(id, display_id, action));
                        ui.ctx().request_repaint_of(ViewportId::ROOT);
                    }
                },
            );
        }
    }

    fn show_pins(&self, ctx: &egui::Context) {
        for (&id, pin) in &self.pins {
            let texture = pin.texture.clone();
            let events = self.sender.clone();
            let prepared = pin.prepared.clone();
            let session = pin.session;
            let placement = pin.placement;
            let display_scale = pin.display_scale;
            let screen = pin.screen;
            let screen_areas = pin.screen_areas.clone();
            let initial_units = Coordinates::new(display_scale, 1.0, display_scale);
            let initial =
                ui::pin::frame_geometry(placement, screen, initial_units.ratio, initial_units.grid);
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of(("pin", id)),
                self.window_icon(egui::ViewportBuilder::default())
                    .with_title("Snip2Pin · 贴图")
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_has_shadow(false)
                    .with_position(initial.window.min)
                    .with_inner_size(initial.window.size())
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_active(false)
                    .with_taskbar(false),
                move |ui, _| {
                    if ui.input(|input| {
                        input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
                    }) {
                        let _ = events.send(Event::ClosePin(id));
                        ui.ctx().request_repaint_of(ViewportId::ROOT);
                        return;
                    }
                    let coordinates = Coordinates::new(
                        display_scale,
                        ui.ctx().zoom_factor(),
                        ui.ctx().pixels_per_point(),
                    );
                    let ratio = coordinates.ratio;
                    let Ok(mut prepared) = prepared.lock() else {
                        return;
                    };
                    let (inner, outer) = ui
                        .input(|input| (input.viewport().inner_rect, input.viewport().outer_rect));
                    let frame = ui::pin::frame_geometry(placement, screen, ratio, coordinates.grid);
                    let (mut frame, first_frame) = match prepared.update(frame, inner, outer, ratio)
                    {
                        Prepared::Initialize {
                            window,
                            outer_position,
                        } => {
                            ui.ctx()
                                .send_viewport_cmd(ViewportCommand::InnerSize(window.size()));
                            ui.ctx()
                                .send_viewport_cmd(ViewportCommand::OuterPosition(outer_position));
                            ui.ctx().request_repaint();
                            return;
                        }
                        Prepared::Paint { frame, first } => (frame, first),
                    };
                    if let Some(actual) = inner {
                        frame.place_controls(actual, &screen_areas, screen, ratio);
                    }
                    ui::pin::show(
                        ui,
                        ui::pin::Model {
                            id,
                            texture: &texture,
                            frame,
                            events: &events,
                        },
                        &mut prepared.drag,
                        platform::pointer_position,
                    );
                    if first_frame {
                        let _ = events.send(Event::PinReady(session, id));
                        ui.ctx().request_repaint_of(ViewportId::ROOT);
                    }
                },
            );
        }
    }
}

impl eframe::App for Snip2Pin {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_icon(ctx);
        platform::prepare_floating_windows();
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Capture => {
                    if Instant::now() >= self.suppress_shortcut_until {
                        self.begin_capture(ctx);
                    }
                }
                Event::Ready(id, result, pointer) => self.accept_capture(id, result, pointer, ctx),
                Event::Selection(id, display_id, action) => {
                    self.selection_action(id, display_id, action, ctx)
                }
                Event::PinReady(session, pin) => {
                    if matches!(&self.phase, Phase::Selecting { id, pending_pin: Some(pending), .. } if *id == session && *pending == pin)
                    {
                        self.restore(ctx);
                    }
                }
                Event::CopyPin(id) => self.copy_pin(id, ctx),
                Event::ClosePin(id) => self.close_pin(id, ctx),
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_icon(&ctx);
        if matches!(self.phase, Phase::Selecting { .. })
            && ui.input(|input| input.key_pressed(egui::Key::Escape))
        {
            self.restore(&ctx);
        }
        let idle = matches!(self.phase, Phase::Idle);
        let action = launcher::show(
            ui,
            launcher::Model {
                pin_count: self.pins.len(),
                message: self.message.as_deref(),
                global_shortcut: self.shortcut.is_some(),
                preferences: self.preferences,
                icon_texture: self.icon_texture.as_ref(),
                recording: self.recording,
                idle,
                demo: self.demo,
            },
        );
        match action {
            Some(launcher::Action::Capture) => self.begin_capture(&ctx),
            Some(launcher::Action::ClosePins) => self.pins.clear(),
            Some(launcher::Action::RecordShortcut) => {
                self.recording = !self.recording;
                self.message = None;
            }
            Some(launcher::Action::Appearance(theme, icon)) => {
                self.change_appearance(theme, icon, &ctx)
            }
            None => {}
        }
        if self.recording {
            self.record_key(ui);
        } else if idle
            && ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| self.preferences.binding.matches(event))
            })
        {
            self.begin_capture(&ctx);
        }
        self.show_pins(&ctx);
        self.show_layers(&ctx);
    }
}

fn layer_id(display_id: u32) -> ViewportId {
    ViewportId::from_hash_of(("selection", display_id))
}

fn texture(ctx: &egui::Context, name: &str, pixels: &image::RgbaImage) -> TextureHandle {
    ctx.load_texture(name, color_image(pixels), egui::TextureOptions::LINEAR)
}

fn color_image(pixels: &image::RgbaImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [pixels.width() as usize, pixels.height() as usize],
        pixels.as_raw(),
    )
}

#[cfg(test)]
mod tests;
