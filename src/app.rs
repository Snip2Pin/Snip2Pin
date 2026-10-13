use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use eframe::egui::{
    self, ColorImage, Pos2, TextureHandle, ViewportCommand, ViewportId, WindowLevel,
};
use snipsprig::{
    capture::{self, Display, Snapshot},
    geometry,
    session::CaptureGate,
};

use crate::{
    platform::{self, Shortcut},
    ui::{self, launcher, overlay},
};

pub enum Event {
    Capture,
    Ready(u64, Result<Snapshot, String>),
    ClosePin(u64),
}

enum Phase {
    Idle,
    Pending {
        id: u64,
        deadline: Instant,
    },
    Capturing,
    Selecting {
        snapshot: Snapshot,
        selection: overlay::Selection,
    },
}

struct Pin {
    texture: TextureHandle,
    size: [f32; 2],
}

pub struct SnipSprig {
    events: Receiver<Event>,
    sender: Sender<Event>,
    phase: Phase,
    gate: CaptureGate,
    pins: BTreeMap<u64, Pin>,
    next_pin: u64,
    displays: Vec<Display>,
    display_index: usize,
    message: Option<String>,
    shortcut: Option<Shortcut>,
    position: Option<Pos2>,
    demo: bool,
}

impl SnipSprig {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        platform::install_fonts(&cc.egui_ctx);
        let (sender, events) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        let hotkey_sender = sender.clone();
        let shortcut = Shortcut::register(move || {
            let _ = hotkey_sender.send(Event::Capture);
            ctx.request_repaint();
        });
        let demo = std::env::args().any(|argument| argument == "--demo");
        let displays = if demo {
            Ok(vec![Display {
                id: 0,
                name: "演示屏幕".into(),
                primary: true,
            }])
        } else {
            capture::displays()
        };
        let message = displays.as_ref().err().cloned();
        let displays = displays.unwrap_or_default();
        let display_index = displays
            .iter()
            .position(|display| display.primary)
            .unwrap_or(0);
        Self {
            events,
            sender,
            phase: Phase::Idle,
            gate: CaptureGate::default(),
            pins: BTreeMap::new(),
            next_pin: 0,
            displays,
            display_index,
            message,
            shortcut: shortcut.ok(),
            position: None,
            demo,
        }
    }

    fn begin_capture(&mut self, ctx: &egui::Context) {
        if !matches!(self.phase, Phase::Idle) {
            return;
        }
        if self.displays.is_empty() {
            self.message = Some("没有可用屏幕，请检查桌面权限并重新打开应用。".into());
            return;
        }
        let Some(id) = self.gate.begin() else {
            return;
        };
        self.message = None;
        self.position = ctx.input(|input| input.viewport().outer_rect.map(|rect| rect.min));
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        for &id in self.pins.keys() {
            ctx.send_viewport_cmd_to(
                ViewportId::from_hash_of(id),
                ViewportCommand::Visible(false),
            );
        }
        self.phase = Phase::Pending {
            id,
            deadline: Instant::now() + Duration::from_millis(220),
        };
        ctx.request_repaint_after(Duration::from_millis(230));
    }

    fn restore(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::Normal));
        let size = if self.message.is_some() {
            egui::vec2(420.0, 340.0)
        } else {
            egui::vec2(380.0, 240.0)
        };
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        if let Some(position) = self.position {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(position));
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        for &id in self.pins.keys() {
            ctx.send_viewport_cmd_to(ViewportId::from_hash_of(id), ViewportCommand::Visible(true));
        }
        self.phase = Phase::Idle;
        ctx.request_repaint();
    }

    fn accept_capture(&mut self, id: u64, result: Result<Snapshot, String>, ctx: &egui::Context) {
        if !self.gate.complete(id) {
            return;
        }
        match result {
            Ok(snapshot) => {
                let pixels = [snapshot.pixels.width(), snapshot.pixels.height()];
                let texture = texture(ctx, "selection", &snapshot.pixels);
                self.phase = Phase::Selecting {
                    snapshot,
                    selection: overlay::Selection {
                        texture,
                        pixels,
                        start: None,
                    },
                };
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
                ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            Err(message) => {
                self.message = Some(message);
                self.restore(ctx);
            }
        }
    }

    fn selection_action(&mut self, action: overlay::Action, ctx: &egui::Context) {
        if let overlay::Action::Select(rect) = action
            && let Phase::Selecting { snapshot, .. } = &self.phase
            && let Some(pixels) = capture::crop(&snapshot.pixels, rect)
        {
            self.next_pin += 1;
            let size = geometry::fit_size(
                [pixels.width(), pixels.height()],
                snapshot.scale,
                [760.0, 520.0],
            );
            let texture = texture(ctx, &format!("pin-{}", self.next_pin), &pixels);
            self.pins.insert(self.next_pin, Pin { texture, size });
        }
        self.restore(ctx);
    }

    fn show_pins(&self, ctx: &egui::Context) {
        for (&id, pin) in &self.pins {
            let texture = pin.texture.clone();
            let events = self.sender.clone();
            let visible = matches!(self.phase, Phase::Idle);
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of(id),
                egui::ViewportBuilder::default()
                    .with_title("SnipSprig · 贴图")
                    .with_decorations(false)
                    .with_inner_size(pin.size)
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_active(false)
                    .with_visible(visible)
                    .with_taskbar(false),
                move |ui, _| ui::pin::show(ui, id, &texture, &events),
            );
        }
    }
}

impl eframe::App for SnipSprig {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Capture => self.begin_capture(ctx),
                Event::Ready(id, result) => self.accept_capture(id, result, ctx),
                Event::ClosePin(id) => {
                    self.pins.remove(&id);
                }
            }
        }
        if let Phase::Pending { id, deadline } = self.phase {
            if Instant::now() >= deadline {
                let display_id = self.displays[self.display_index].id;
                let sender = self.sender.clone();
                let ctx = ctx.clone();
                let demo = self.demo;
                self.phase = Phase::Capturing;
                std::thread::spawn(move || {
                    let result = if demo {
                        Ok(capture::demo())
                    } else {
                        capture::monitor(display_id)
                    };
                    let _ = sender.send(Event::Ready(id, result));
                    ctx.request_repaint();
                });
            } else {
                ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Phase::Selecting { selection, .. } = &mut self.phase {
            if let Some(action) = overlay::show(ui, selection) {
                self.selection_action(action, &ctx);
            }
        } else if matches!(self.phase, Phase::Idle) {
            let action = launcher::show(
                ui,
                &self.displays,
                &mut self.display_index,
                self.pins.len(),
                self.message.as_deref(),
                self.shortcut.is_some(),
                self.demo,
            );
            if let Some(action) = action {
                match action {
                    launcher::Action::Capture => self.begin_capture(&ctx),
                    launcher::Action::ClosePins => self.pins.clear(),
                }
            }
            if ui.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key { key, physical_key, pressed: true, modifiers, .. }
                        if modifiers.command && modifiers.shift
                            && (*key == egui::Key::Num2 || *physical_key == Some(egui::Key::Num2))
                    )
                })
            }) {
                self.begin_capture(&ctx);
            }
        }
        self.show_pins(&ctx);
    }
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
