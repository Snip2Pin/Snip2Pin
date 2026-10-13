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
use snipsprig::{
    capture::{self, Display, Snapshot},
    geometry::{self, Placement},
    session::CaptureGate,
    settings::{self, Binding},
};

use crate::{
    platform::{self, Shortcut},
    ui::{self, launcher, overlay},
};

pub enum Event {
    Capture,
    Ready(u64, Result<Vec<Snapshot>, String>),
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

pub struct SnipSprig {
    events: Receiver<Event>,
    sender: Sender<Event>,
    phase: Phase,
    gate: CaptureGate,
    layers: BTreeMap<u32, Layer>,
    pins: BTreeMap<u64, Pin>,
    next_pin: u64,
    message: Option<String>,
    shortcut: Option<Shortcut>,
    binding: Binding,
    config_path: Option<PathBuf>,
    recording: bool,
    suppress_shortcut_until: Instant,
    demo: bool,
}

impl SnipSprig {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        platform::install_fonts(&cc.egui_ctx);
        let demo = std::env::args().any(|argument| argument == "--demo");
        let (sender, events) = mpsc::channel();
        let config_path = (!demo).then(settings::path).flatten();
        let loaded = config_path
            .as_deref()
            .map(Binding::load)
            .unwrap_or(Ok(Binding::default()));
        let mut message = loaded.as_ref().err().cloned();
        let binding = loaded.unwrap_or_default();
        let shortcut = if demo {
            None
        } else {
            let ctx = cc.egui_ctx.clone();
            let hotkey_sender = sender.clone();
            match Shortcut::register(binding, move || {
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
            pins: BTreeMap::new(),
            next_pin: 0,
            message,
            shortcut,
            binding,
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
        std::thread::spawn(move || {
            let result = if demo {
                Ok(vec![capture::demo()])
            } else {
                capture::desktop()
            };
            let _ = sender.send(Event::Ready(id, result));
            ctx.request_repaint_of(ViewportId::ROOT);
        });
    }

    fn restore(&mut self, ctx: &egui::Context) {
        let was_selecting = matches!(self.phase, Phase::Selecting { .. });
        for layer in self.layers.values_mut() {
            layer.selection = None;
        }
        self.phase = Phase::Idle;
        if was_selecting {
            platform::finish_capture(ctx);
        }
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn accept_capture(
        &mut self,
        id: u64,
        result: Result<Vec<Snapshot>, String>,
        ctx: &egui::Context,
    ) {
        if !self.gate.complete(id) {
            return;
        }
        match result {
            Ok(snapshots) if !snapshots.is_empty() => {
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
        if let Some(path) = &self.config_path
            && let Err(error) = binding.save(path)
        {
            let rollback = self
                .shortcut
                .as_mut()
                .map(|shortcut| shortcut.replace(self.binding));
            if matches!(rollback, Some(Err(_))) {
                // Show the actual registered binding if rollback fails.
                self.binding = binding;
            }
            self.message = Some(error);
            return;
        }
        self.binding = binding;
        self.message = Some(
            if self.shortcut.is_some() {
                "快捷键已保存。"
            } else {
                "快捷键已保存；当前桌面仅支持窗口内快捷键。"
            }
            .into(),
        );
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
        for (&display_id, layer) in &self.layers {
            let selection = layer.selection.clone();
            let events = self.sender.clone();
            let active = selection.is_some();
            let display = layer.display.clone();
            let position = display.position();
            let size = display.logical_size();
            ctx.show_viewport_deferred(
                layer_id(display_id),
                egui::ViewportBuilder::default()
                    .with_title("SnipSprig · 框选")
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
                        && let Some(action) = overlay::show(ui, &mut selection)
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
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of(("pin", id)),
                egui::ViewportBuilder::default()
                    .with_title("SnipSprig · 贴图")
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_has_shadow(false)
                    .with_position(placement.position)
                    .with_inner_size(placement.size)
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_active(false)
                    .with_taskbar(false),
                move |ui, _| {
                    if prepared.load(Ordering::Acquire) == 0 {
                        let ratio = if cfg!(target_os = "macos") {
                            1.0
                        } else {
                            display_scale / ui.ctx().pixels_per_point()
                        };
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(
                                placement.position[0] * ratio,
                                placement.position[1] * ratio,
                            )));
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(
                                placement.size[0] * ratio,
                                placement.size[1] * ratio,
                            )));
                        prepared.store(1, Ordering::Release);
                        ui.ctx().request_repaint();
                        return;
                    }
                    ui::pin::show(ui, id, &texture, &events);
                    if prepared.swap(2, Ordering::AcqRel) == 1 {
                        let _ = events.send(Event::PinReady(session, id));
                        ui.ctx().request_repaint_of(ViewportId::ROOT);
                    }
                },
            );
        }
    }
}

impl eframe::App for SnipSprig {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        platform::prepare_floating_windows();
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Capture => {
                    if Instant::now() >= self.suppress_shortcut_until {
                        self.begin_capture(ctx);
                    }
                }
                Event::Ready(id, result) => self.accept_capture(id, result, ctx),
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
        let idle = matches!(self.phase, Phase::Idle);
        let action = launcher::show(
            ui,
            launcher::Model {
                pin_count: self.pins.len(),
                message: self.message.as_deref(),
                global_shortcut: self.shortcut.is_some(),
                binding: self.binding,
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
            None => {}
        }
        if self.recording {
            self.record_key(ui);
        } else if idle
            && ui.input(|input| input.events.iter().any(|event| self.binding.matches(event)))
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

    fn app() -> SnipSprig {
        let (sender, events) = mpsc::channel();
        SnipSprig {
            sender,
            events,
            phase: Phase::Idle,
            gate: CaptureGate::default(),
            layers: BTreeMap::new(),
            pins: BTreeMap::new(),
            next_pin: 0,
            message: None,
            shortcut: None,
            binding: Binding::default(),
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
            app.accept_capture(id, Ok(vec![capture::demo()]), ui.ctx());
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
    fn every_monitor_gets_a_layer_and_pin_uses_that_monitors_origin() {
        let ctx = egui::Context::default();
        let mut app = app();
        let mut left = capture::demo();
        left.display.id = 9;
        left.display.origin = [-680.0, -100.0];
        let origin = left.display.position();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo(), left]), &ctx);
        assert_eq!(app.layers.len(), 2);
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
        assert!(app.layers.values().all(|layer| layer.selection.is_none()));
        assert_eq!(app.pins.len(), 1);
    }

    #[test]
    fn canceled_and_stale_selections_release_buffers_without_creating_pins() {
        let ctx = egui::Context::default();
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo()]), &ctx);
        app.selection_action(id + 1, 0, overlay::Action::Cancel, &ctx);
        assert!(matches!(app.phase, Phase::Selecting { .. }));
        app.selection_action(id, 0, overlay::Action::Cancel, &ctx);
        assert!(matches!(app.phase, Phase::Idle));
        assert!(app.layers.values().all(|layer| layer.selection.is_none()));
        assert!(app.pins.is_empty());
    }

    #[test]
    fn pin_window_is_created_at_the_selection_origin_without_changing_root() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        app.accept_capture(id, Ok(vec![capture::demo()]), &ctx);
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
        assert_eq!(pin.builder.position, Some(egui::pos2(148.0, 142.0)));
        assert_eq!(pin.builder.inner_size, Some(egui::vec2(136.0, 84.0)));
        assert!(
            output.viewport_output[&ViewportId::ROOT]
                .commands
                .iter()
                .all(|command| matches!(command, ViewportCommand::SetTheme(_))),
            "unexpected root commands: {:?}",
            output.viewport_output[&ViewportId::ROOT].commands
        );
    }
}
