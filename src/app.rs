use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

use eframe::egui::{self, ColorImage, TextureHandle, ViewportCommand, ViewportId};
use snip2pin::{
    capture::{self, Display, Snapshot},
    geometry::{self, Placement},
    icon::{self, Variant},
    session::CaptureGate,
    settings::{self, Binding, Preferences},
};

use crate::{
    platform::{self, Shortcut},
    ui::{self, launcher, overlay},
};

pub enum Event {
    Capture,
    Ready(u64, Result<Vec<Snapshot>, String>, Option<[f32; 2]>),
    Selection(u64, u32, overlay::Action),
    PinReady(u64, u64),
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
}

struct Pin {
    texture: TextureHandle,
    placement: Placement,
    display_scale: f32,
    session: u64,
    prepared: Arc<AtomicU8>,
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
        if id != *current || pending_pin.is_some() {
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
            let texture = texture(ctx, &format!("pin-{}", self.next_pin), &pixels);
            self.pins.insert(
                self.next_pin,
                Pin {
                    texture,
                    placement,
                    display_scale: snapshot.display.scale,
                    session: id,
                    prepared: Arc::new(AtomicU8::new(0)),
                },
            );
            // Keep the selection image until the pin has painted its first frame.
            *pending_pin = Some(self.next_pin);
            ctx.request_repaint_of(ViewportId::ROOT);
        } else {
            self.restore(ctx);
        }
    }

    fn change_binding(&mut self, binding: Binding, ctx: &egui::Context) {
        self.suppress_shortcut_until = Instant::now() + Duration::from_millis(200);
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
        match icon::render(variant, 64) {
            Ok(pixels) => {
                self.icon_texture = Some(texture(ctx, "application-icon", &pixels));
                let data = Arc::new(egui::IconData {
                    rgba: pixels.into_raw(),
                    width: 64,
                    height: 64,
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
                        if (actual.x - expected.x).abs() > 0.5
                            || (actual.y - expected.y).abs() > 0.5
                        {
                            ui.ctx().request_repaint();
                            return;
                        }
                    }
                    // Correct initial placement after the native window resolves
                    // its actual monitor/DPI (especially mixed-DPI Windows).
                    if !cfg!(target_os = "macos") {
                        let scale = ui.ctx().pixels_per_point();
                        let target =
                            egui::pos2(display.origin[0] / scale, display.origin[1] / scale);
                        let actual =
                            ui.input(|input| input.viewport().outer_rect.map(|rect| rect.min));
                        if actual.is_some_and(|actual| actual.distance(target) > 0.5) {
                            ui.ctx()
                                .send_viewport_cmd(ViewportCommand::OuterPosition(target));
                        }
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
            let (initial, _) = ui::pin::frame_geometry(
                placement,
                1.0,
                if cfg!(target_os = "macos") {
                    1.0
                } else {
                    display_scale
                },
            );
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of(("pin", id)),
                self.window_icon(egui::ViewportBuilder::default())
                    .with_title("Snip2Pin · 贴图")
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_has_shadow(false)
                    .with_position(initial.min)
                    .with_inner_size(initial.size())
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_active(false)
                    .with_taskbar(false),
                move |ui, _| {
                    let ratio = if cfg!(target_os = "macos") {
                        ui.ctx().zoom_factor().recip()
                    } else {
                        display_scale / ui.ctx().pixels_per_point()
                    };
                    let grid = if cfg!(target_os = "macos") {
                        ui.ctx().zoom_factor()
                    } else {
                        ui.ctx().pixels_per_point()
                    };
                    let (window, image) = ui::pin::frame_geometry(placement, ratio, grid);
                    if prepared.load(Ordering::Acquire) < 2 {
                        let position = window.min;
                        let size = window.size();
                        let (inner, outer) = ui.input(|input| {
                            (input.viewport().inner_rect, input.viewport().outer_rect)
                        });
                        let tolerance = 0.51 / ui.ctx().pixels_per_point();
                        let size_matches = (ui.max_rect().width() - size.x).abs() <= tolerance
                            && (ui.max_rect().height() - size.y).abs() <= tolerance;
                        let position_matches = match inner {
                            Some(rect) => {
                                (rect.min.x - position.x).abs() <= tolerance
                                    && (rect.min.y - position.y).abs() <= tolerance
                            }
                            // Native Wayland does not expose desktop positions.
                            None => {
                                cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none()
                            }
                        };
                        if prepared.load(Ordering::Acquire) == 0
                            || !size_matches
                            || !position_matches
                        {
                            // Match the client image, including native invisible
                            // frame insets. Resize before moving, then wait for
                            // the native geometry event before revealing the pin.
                            let inset = inner
                                .zip(outer)
                                .map_or(egui::Vec2::ZERO, |(inner, outer)| inner.min - outer.min);
                            ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(size));
                            ui.ctx().send_viewport_cmd(ViewportCommand::OuterPosition(
                                position - inset,
                            ));
                            prepared.store(1, Ordering::Release);
                            ui.ctx().request_repaint();
                            return;
                        }
                    }
                    ui::pin::show(ui, id, &texture, image, &events);
                    if prepared.swap(2, Ordering::AcqRel) == 1 {
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
                Event::ClosePin(id) => {
                    self.pins.remove(&id);
                }
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
    ctx.load_texture(
        name,
        ColorImage::from_rgba_unmultiplied(
            [pixels.width() as usize, pixels.height() as usize],
            pixels.as_raw(),
        ),
        egui::TextureOptions::LINEAR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> Snip2Pin {
        let (sender, events) = mpsc::channel();
        Snip2Pin {
            sender,
            events,
            phase: Phase::Idle,
            gate: CaptureGate::default(),
            layers: BTreeMap::new(),
            active_display: None,
            pins: BTreeMap::new(),
            next_pin: 0,
            message: None,
            shortcut: None,
            preferences: Preferences::default(),
            applied_icon: None,
            icon_data: None,
            icon_texture: None,
            config_path: None,
            recording: false,
            suppress_shortcut_until: Instant::now(),
            demo: true,
        }
    }

    #[test]
    fn capture_never_switches_the_desktop_into_native_fullscreen() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.accept_capture(id, Ok(vec![capture::demo()]), None, ui.ctx());
            app.show_layers(ui.ctx());
            app.restore(ui.ctx());
        });
        output.textures_delta.clear();
        assert!(
            output.viewport_output.values().all(|viewport| {
                viewport.builder.fullscreen != Some(true)
                    && viewport.builder.monitor.is_none()
                    && !viewport.commands.iter().any(|command| {
                        matches!(
                            command,
                            ViewportCommand::Fullscreen(_) | ViewportCommand::SetMonitor(_)
                        )
                    })
            }),
            "capture must never enter/leave native fullscreen or switch monitors"
        );
    }

    #[test]
    fn capture_initially_highlights_the_display_under_the_cursor() {
        let ctx = egui::Context::default();
        let mut app = app();
        let mut left = capture::demo();
        left.display.id = 9;
        left.display.origin = [-680.0, -100.0];
        left.display.scale = 2.0;
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(
            id,
            Ok(vec![capture::demo(), left]),
            Some([-300.0, -50.0]),
            &ctx,
        );
        assert_eq!(
            app.active_display,
            Some(9),
            "the pointer chooses the screen, not display ordering or the launcher"
        );
    }

    #[test]
    fn every_monitor_gets_a_layer_and_pin_uses_that_monitors_origin() {
        let ctx = egui::Context::default();
        let mut app = app();
        let mut left = capture::demo();
        left.display.id = 9;
        left.display.origin = [-680.0, -100.0];
        let origin = left.display.position();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo(), left]), None, &ctx);
        assert_eq!(app.layers.len(), 2);
        assert_eq!(app.active_display, Some(0));
        app.selection_action(id, 9, overlay::Action::Activate, &ctx);
        assert_eq!(app.active_display, Some(9));
        app.selection_action(
            id,
            9,
            overlay::Action::Select(geometry::PixelRect {
                x: 96,
                y: 60,
                width: 192,
                height: 120,
            }),
            &ctx,
        );
        let pin = app.pins.values().next().expect("pin created");
        assert_eq!(pin.placement.position, [origin[0] + 68.0, origin[1] + 42.0]);
        assert_eq!(pin.placement.size, [136.0, 84.0]);
        assert!(matches!(
            app.phase,
            Phase::Selecting {
                pending_pin: Some(_),
                ..
            }
        ));
        app.restore(&ctx);
        assert_eq!(app.active_display, None);
        assert!(app.layers.values().all(|layer| layer.selection.is_none()));
        assert_eq!(app.pins.len(), 1);
    }

    #[test]
    fn canceled_and_stale_selections_release_buffers_without_creating_pins() {
        let ctx = egui::Context::default();
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
        app.selection_action(id + 1, 0, overlay::Action::Cancel, &ctx);
        assert!(matches!(app.phase, Phase::Selecting { .. }));
        app.selection_action(id, 0, overlay::Action::Cancel, &ctx);
        assert!(matches!(app.phase, Phase::Idle));
        assert!(app.layers.values().all(|layer| layer.selection.is_none()));
        assert!(app.pins.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn menu_bar_constrained_layer_waits_before_painting_the_full_desktop() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut setup = ctx.run_ui(
            egui::RawInput {
                max_texture_side: Some(4096),
                ..Default::default()
            },
            |_| {},
        );
        setup.textures_delta.clear();
        let mut app = app();
        app.demo = false;
        let mut snapshot = capture::demo();
        snapshot.display.size = [1440.0, 900.0];
        snapshot.pixels = image::RgbaImage::new(2880, 1800);
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
        let layer = layer_id(0);
        let texture = app.layers[&0]
            .selection
            .as_ref()
            .expect("selection")
            .lock()
            .expect("selection lock")
            .texture
            .id();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
        output.textures_delta.clear();
        let paint = output.viewport_output[&layer]
            .viewport_ui_cb
            .as_ref()
            .expect("native layer callback");
        for (size, should_paint) in [([1440.0, 870.0], false), ([1440.0, 900.0], true)] {
            let mut input = egui::RawInput {
                viewport_id: layer,
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size.into())),
                ..Default::default()
            };
            input.viewports.insert(layer, egui::ViewportInfo::default());
            let mut frame = ctx.run_ui(input, |ui| paint(ui));
            frame.textures_delta.clear();
            let images: Vec<_> = frame
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                        Some(mesh.calc_bounds())
                    }
                    _ => None,
                })
                .collect();
            if should_paint {
                assert_eq!(
                    images,
                    vec![egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 900.0)
                    )],
                    "Retina desktop must retain its full dimensions and aspect ratio"
                );
            } else {
                assert!(
                    images.is_empty(),
                    "never paint a vertically compressed desktop"
                );
            }
        }
    }

    #[test]
    fn misaligned_pin_must_not_paint_or_finish_capture() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
        app.selection_action(
            id,
            0,
            overlay::Action::Select(geometry::PixelRect {
                x: 96,
                y: 60,
                width: 192,
                height: 120,
            }),
            &ctx,
        );
        let viewport = ViewportId::from_hash_of(("pin", 1_u64));
        let texture = app.pins[&1].texture.id();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
        output.textures_delta.clear();
        let paint = output.viewport_output[&viewport]
            .viewport_ui_cb
            .as_ref()
            .expect("pin callback");
        for _ in 0..2 {
            let rect =
                egui::Rect::from_min_size(egui::pos2(142.0, 135.0), egui::vec2(156.0, 104.0));
            let mut input = egui::RawInput {
                viewport_id: viewport,
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size())),
                ..Default::default()
            };
            input.viewports.insert(
                viewport,
                egui::ViewportInfo {
                    inner_rect: Some(rect),
                    outer_rect: Some(rect),
                    ..Default::default()
                },
            );
            let mut frame = ctx.run_ui(input, |ui| paint(ui));
            frame.textures_delta.clear();
            assert!(
                frame.shapes.iter().all(|shape| {
                    !matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == texture)
                }),
                "do not reveal a pin a few pixels away from the selected desktop region"
            );
            assert!(
                app.events
                    .try_iter()
                    .all(|event| !matches!(event, Event::PinReady(_, _))),
                "keep the selection visible until native placement is confirmed"
            );
        }
    }

    #[test]
    fn aligned_pin_keeps_image_origin_across_dpi_zoom_and_native_frame_insets() {
        for scale in [1.0, 1.25, 2.0] {
            for zoom in [1.0, 1.25] {
                for origin in [[80.0, 100.0], [-680.0, -100.0]] {
                    let ctx = egui::Context::default();
                    ctx.set_embed_viewports(false);
                    ctx.set_zoom_factor(zoom);
                    let mut app = app();
                    let id = app.gate.begin().expect("capture starts");
                    let mut snapshot = capture::demo();
                    snapshot.display.origin = origin;
                    snapshot.display.scale = scale;
                    if cfg!(target_os = "macos") {
                        snapshot.display.size = [960.0 / scale, 600.0 / scale];
                    } else {
                        snapshot.display.origin = origin.map(|value| value * scale);
                        snapshot.display.size = [960.0, 600.0];
                    }
                    app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
                    app.selection_action(
                        id,
                        0,
                        overlay::Action::Select(geometry::PixelRect {
                            x: 97,
                            y: 61,
                            width: 193,
                            height: 121,
                        }),
                        &ctx,
                    );
                    let viewport = ViewportId::from_hash_of(("pin", 1_u64));
                    let pin = &app.pins[&1];
                    let texture = pin.texture.id();
                    let position =
                        egui::pos2(pin.placement.position[0], pin.placement.position[1]) / zoom;
                    let size = egui::vec2(pin.placement.size[0], pin.placement.size[1]) / zoom;
                    let grid = if cfg!(target_os = "macos") {
                        zoom
                    } else {
                        scale * zoom
                    };
                    let (window, _) = ui::pin::frame_geometry(pin.placement, zoom.recip(), grid);
                    let inset = egui::vec2(4.0, 7.0) / (scale * zoom);
                    let mut output =
                        ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
                    output.textures_delta.clear();
                    let paint = output.viewport_output[&viewport]
                        .viewport_ui_cb
                        .as_ref()
                        .expect("pin callback");
                    let input = |rect: egui::Rect| {
                        let mut input = egui::RawInput {
                            viewport_id: viewport,
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                rect.size(),
                            )),
                            ..Default::default()
                        };
                        input.viewports.insert(
                            viewport,
                            egui::ViewportInfo {
                                native_pixels_per_point: Some(scale),
                                inner_rect: Some(rect),
                                outer_rect: Some(rect.translate(-inset)),
                                ..Default::default()
                            },
                        );
                        input
                    };
                    let mut first = ctx.run_ui(input(window), |ui| paint(ui));
                    first.textures_delta.clear();
                    let commands = &first.viewport_output[&viewport].commands;
                    let resize = commands
                        .iter()
                        .position(|command| matches!(command, ViewportCommand::InnerSize(_)))
                        .expect("resize");
                    let move_window = commands
                        .iter()
                        .position(|command| matches!(command, ViewportCommand::OuterPosition(_)))
                        .expect("position");
                    assert!(
                        resize < move_window,
                        "resizing must not undo the final top-left position"
                    );
                    assert!(commands.iter().any(|command| matches!(command,
                        ViewportCommand::OuterPosition(actual) if actual.distance(window.min - inset) < 0.001
                    )), "position targets the client area, compensating native insets");
                    let mut frame = ctx.run_ui(input(window), |ui| paint(ui));
                    frame.textures_delta.clear();
                    let image = frame
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                                Some(mesh.calc_bounds())
                            }
                            _ => None,
                        })
                        .expect("aligned image is painted");
                    let tolerance = 0.001;
                    assert!((window.min + image.min.to_vec2()).distance(position) < tolerance);
                    assert!(
                        (image.width() - size.x).abs() < tolerance
                            && (image.height() - size.y).abs() < tolerance,
                        "native window rounding must not stretch the image"
                    );
                    assert!(
                        app.events
                            .try_iter()
                            .any(|event| matches!(event, Event::PinReady(_, 1)))
                    );
                    let mut dragged = ctx
                        .run_ui(input(window.translate(egui::vec2(20.0, 30.0))), |ui| {
                            paint(ui)
                        });
                    dragged.textures_delta.clear();
                    assert!(
                        dragged.viewport_output[&viewport]
                            .commands
                            .iter()
                            .all(|command| !matches!(command, ViewportCommand::OuterPosition(_))),
                        "once initialized, user dragging must not snap the pin back"
                    );
                }
            }
        }
    }

    #[test]
    fn pin_window_is_created_at_the_selection_origin_without_changing_root() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
        app.selection_action(
            id,
            0,
            overlay::Action::Select(geometry::PixelRect {
                x: 96,
                y: 60,
                width: 192,
                height: 120,
            }),
            &ctx,
        );
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.show_pins(ui.ctx());
        });
        output.textures_delta.clear();
        let pin = output
            .viewport_output
            .get(&ViewportId::from_hash_of(("pin", 1_u64)))
            .expect("native pin");
        assert_eq!(pin.builder.position, Some(egui::pos2(138.0, 132.0)));
        assert_eq!(pin.builder.inner_size, Some(egui::vec2(156.0, 104.0)));
        assert!(
            output.viewport_output[&ViewportId::ROOT]
                .commands
                .iter()
                .all(|command| matches!(command, ViewportCommand::SetTheme(_))),
            "unexpected root commands: {:?}",
            output.viewport_output[&ViewportId::ROOT].commands
        );
    }

    #[test]
    fn ready_and_cancel_repaint_warmed_selection_layers_without_pointer_events() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        app.layers.insert(
            0,
            Layer {
                display: capture::demo().display,
                selection: None,
            },
        );
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
        output.textures_delta.clear();
        let drain = || {
            for _ in 0..4 {
                let mut input = egui::RawInput {
                    viewport_id: layer_id(0),
                    ..Default::default()
                };
                input.viewports.insert(
                    layer_id(0),
                    egui::ViewportInfo {
                        parent: Some(ViewportId::ROOT),
                        ..Default::default()
                    },
                );
                let mut output = ctx.run_ui(input, |_| {});
                output.textures_delta.clear();
            }
        };
        drain();
        let repaints = Arc::new(Mutex::new(Vec::new()));
        let observed = repaints.clone();
        ctx.set_request_repaint_callback(move |info| {
            observed.lock().expect("repaint log").push(info.viewport_id);
        });
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
        assert!(
            repaints.lock().expect("repaint log").contains(&layer_id(0)),
            "ready must paint the selection without waiting for pointer input"
        );
        drain();
        repaints.lock().expect("repaint log").clear();
        app.restore(&ctx);
        assert!(
            repaints.lock().expect("repaint log").contains(&layer_id(0)),
            "cancel must clear the selection without waiting for pointer input"
        );
    }

    #[test]
    fn appearance_changes_preserve_shortcut_and_survive_restart() {
        let directory = tempfile::tempdir().expect("temporary preferences");
        let path = directory.path().join("preferences.toml");
        let ctx = egui::Context::default();
        let mut app = app();
        app.config_path = Some(path.clone());
        app.preferences.binding = Binding::parse("Ctrl+Alt+K").expect("custom binding");
        app.change_appearance(settings::Theme::Dark, settings::Icon::Line, &ctx);
        assert_eq!(
            app.preferences.binding,
            Binding::parse("Ctrl+Alt+K").expect("preserved binding")
        );
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(Preferences::load(&path).expect("restart"), app.preferences);
        app.change_binding(Binding::parse("Ctrl+Shift+4").expect("new binding"), &ctx);
        let reloaded = Preferences::load(&path).expect("restart after shortcut change");
        assert_eq!(reloaded.theme, settings::Theme::Dark);
        assert_eq!(reloaded.icon, settings::Icon::Line);
        assert_eq!(
            reloaded.binding,
            Binding::parse("Ctrl+Shift+4").expect("updated binding")
        );
    }

    #[test]
    fn failed_preference_save_leaves_theme_and_icon_unchanged() {
        let directory = tempfile::tempdir().expect("temporary preferences");
        let ctx = egui::Context::default();
        let mut app = app();
        app.config_path = Some(directory.path().to_path_buf());
        let previous = app.preferences;
        let previous_theme = ctx.theme();
        app.change_appearance(settings::Theme::Light, settings::Icon::Metal, &ctx);
        assert_eq!(app.preferences, previous);
        assert_eq!(ctx.theme(), previous_theme);
        assert!(app.message.is_some());
        assert_eq!(
            std::fs::read_dir(directory.path())
                .expect("directory preserved")
                .count(),
            0
        );
    }
}
