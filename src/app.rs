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
use raw_window_handle::HasWindowHandle;
use snip2pin::{
    capture::{self, Display, Snapshot},
    geometry,
    i18n::{self, Language, Locale, Message, Text},
    icon::{self, Variant},
    permission::ScreenPermission,
    pin,
    session::CaptureGate,
    settings::{self, Binding, Preferences},
};
#[cfg(target_os = "macos")]
use std::cell::Cell;

use crate::{
    platform::{self, Shortcut},
    ui::{self, launcher, overlay},
};

const SHORTCUT_SUPPRESSION: Duration = Duration::from_millis(200);
const OVERLAY_SIZE_TOLERANCE: f32 = 0.5;

fn pin_id(id: u64) -> ViewportId {
    ViewportId::from_hash_of(("pin", id))
}

fn pin_hint_state_id(id: u64) -> egui::Id {
    egui::Id::new(("pin-small-hint", id))
}

fn pin_menu_id() -> ViewportId {
    ViewportId::from_hash_of("pin-menu")
}

fn pin_event(
    action: ui::pin::Action,
    ctx: &egui::Context,
    menu_position: Option<egui::Pos2>,
) -> Event {
    match action {
        ui::pin::Action::UnlockAll => Event::Tray(crate::tray::Action::UnlockPins),
        ui::pin::Action::UnlockHint(id) => Event::UnlockHint(id),
        ui::pin::Action::CopyPin(id) => Event::CopyPin(id),
        ui::pin::Action::ClosePin(id) => Event::ClosePin(id),
        ui::pin::Action::MenuPin(id) => Event::MenuPin {
            pin_id: id,
            position: menu_position,
        },
        ui::pin::Action::ToggleLock(id) => Event::ToggleLock(id),
        ui::pin::Action::Zoom(id, commands) => {
            let observed = (ctx.viewport_id() == pin_id(id))
                .then(|| {
                    ctx.native_pixels_per_point().map(|scale| ObservedSize {
                        content_size: ctx.content_rect().size() * ctx.zoom_factor(),
                        native_pixels_per_point: scale,
                    })
                })
                .flatten();
            Event::ZoomPin {
                pin_id: id,
                commands,
                observed,
            }
        }
        ui::pin::Action::Move(id, steps) => {
            let observed = (ctx.viewport_id() == pin_id(id))
                .then(|| {
                    ctx.input(|input| {
                        input
                            .viewport()
                            .outer_rect
                            .zip(input.viewport().native_pixels_per_point)
                    })
                    .map(|(rect, scale)| ObservedPosition {
                        outer_origin: rect.min * ctx.zoom_factor(),
                        native_pixels_per_point: scale,
                    })
                })
                .flatten();
            Event::MovePin {
                pin_id: id,
                steps,
                observed,
            }
        }
    }
}

fn dispatch_pin(
    action: ui::pin::Action,
    events: &Sender<Event>,
    ctx: &egui::Context,
    menu_position: Option<egui::Pos2>,
) {
    let _ = events.send(pin_event(action, ctx, menu_position));
    ctx.request_repaint_of(ViewportId::ROOT);
}

#[derive(Clone, Copy)]
pub struct ObservedSize {
    /// Content size in native window points.
    pub content_size: egui::Vec2,
    pub native_pixels_per_point: f32,
}

#[derive(Clone, Copy)]
pub struct ObservedPosition {
    /// Outer origin in native window points.
    pub outer_origin: egui::Pos2,
    pub native_pixels_per_point: f32,
}

pub enum Event {
    UnlockHint(u64),
    HintError(Message),
    Capture,
    Ready {
        request_id: u64,
        result: Result<Vec<Snapshot>, Message>,
        pointer: Option<[f32; 2]>,
    },
    Selection {
        request_id: u64,
        display_id: u32,
        action: overlay::Action,
    },
    PinFrameDrawn {
        session: u64,
        pin_id: u64,
    },
    #[cfg(target_os = "macos")]
    BeginPinDrag {
        pin_id: u64,
        press: platform::macos_drag::PressId,
    },
    CopyPin(u64),
    ClosePin(u64),
    ToggleLock(u64),
    ZoomPin {
        pin_id: u64,
        commands: Vec<pin::Zoom>,
        observed: Option<ObservedSize>,
    },
    MovePin {
        pin_id: u64,
        steps: [i32; 2],
        observed: Option<ObservedPosition>,
    },
    MenuPin {
        pin_id: u64,
        position: Option<egui::Pos2>,
    },
    ClosePinMenu,
    Tray(crate::tray::Action),
    QuitDecision {
        request_id: u64,
        outcome: Result<bool, Message>,
    },
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

struct UnlockHint {
    pin: u64,
    areas: BTreeMap<u32, egui::Rect>,
    until: Instant,
    raised: Arc<AtomicBool>,
}

struct Pin {
    name: String,
    zoom: platform::pin_zoom::PinZoom,
    texture: TextureHandle,
    pixels: Arc<ColorImage>,
    initial: egui::Rect,
    display_scale: f32,
    screen: egui::Rect,
    screen_areas: Arc<Vec<egui::Rect>>,
    session: u64,
    // The UI callback has drawn the image; compositor presentation is not observed.
    first_frame_drawn: Arc<AtomicBool>,
    locked: bool,
    #[cfg(target_os = "macos")]
    lock_limits: Option<platform::PinLock>,
    #[cfg(target_os = "macos")]
    // Visible(true) has been sent after hidden initialization.
    show_requested: Cell<bool>,
}

impl Pin {
    fn release(self, id: u64, ctx: &egui::Context) {
        ui::pin::forget(ctx, id);
        ctx.data_mut(|data| data.remove::<bool>(pin_hint_state_id(id)));
        if self.first_frame_drawn.load(Ordering::Acquire) {
            ctx.send_viewport_cmd_to(pin_id(id), ViewportCommand::Close);
        }
    }
}

struct PinMenu {
    pin_id: u64,
    position: Option<egui::Pos2>,
    focused: Arc<AtomicBool>,
}

struct QuitConfirmation {
    id: u64,
    presentation: QuitPresentation,
}

enum QuitPresentation {
    AwaitingHost,
    Ready,
    Native(platform::quit_prompt::Dialog),
    Fallback,
}

pub struct Snip2Pin {
    events: Receiver<Event>,
    sender: Sender<Event>,
    phase: Phase,
    gate: CaptureGate,
    capture_worker: Option<capture::Worker>,
    layers: BTreeMap<u32, Layer>,
    active_display: Option<u32>,
    pins: BTreeMap<u64, Pin>,
    pin_menu: Option<PinMenu>,
    unlock_hint: Option<UnlockHint>,
    quit_confirmation: Option<QuitConfirmation>,
    next_quit_request: u64,
    #[cfg(target_os = "macos")]
    pin_drag: platform::macos_drag::DragEvents,
    next_pin: u64,
    message: Option<Message>,
    system_locale: Locale,
    shortcut: Option<Shortcut>,
    preferences: Preferences,
    applied_icon: Option<Variant>,
    icon_data: Option<Arc<egui::IconData>>,
    icon_texture: Option<TextureHandle>,
    config_path: Option<PathBuf>,
    recording: bool,
    suppress_shortcut_until: Instant,
    demo: bool,
    tray: Option<crate::tray::Tray>,
    #[cfg(target_os = "macos")]
    dock: Option<crate::macos_dock::Dock>,
    startup: Option<crate::startup::Startup>,
    startup_enabled: bool,
    exiting: bool,
    home_visible: bool,
    booting: bool,
    permission: ScreenPermission,
    relaunch: Arc<Mutex<Option<std::process::Command>>>,
}

impl Snip2Pin {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        relaunch: Arc<Mutex<Option<std::process::Command>>>,
    ) -> Self {
        platform::install_fonts(&cc.egui_ctx);
        let demo = std::env::args().any(|argument| argument == "--demo");
        let (sender, events) = mpsc::channel();
        let config_path = (!demo).then(settings::path).flatten();
        let loaded = config_path
            .as_deref()
            .map(Preferences::load)
            .unwrap_or(Ok(Preferences::default()));
        let mut message = loaded.as_ref().err().cloned();
        let mut preferences = loaded.unwrap_or_default();
        if let Err(error) = preferences.binding.validate_capture() {
            preferences.binding = Binding::default();
            message = Some(error);
        }
        let system_locale = Locale::system();
        let locale = preferences.language.resolve(system_locale);
        i18n::set_locale(&cc.egui_ctx, locale);
        cc.egui_ctx.send_viewport_cmd_to(
            ViewportId::ROOT,
            ViewportCommand::Title(locale.text(Text::WindowTitle).into()),
        );
        cc.egui_ctx
            .options_mut(|options| options.fallback_theme = egui::Theme::Light);
        cc.egui_ctx.set_theme(preferences.theme.preference());
        let shortcut = if demo {
            None
        } else {
            match register_shortcut(preferences.binding, &sender, &cc.egui_ctx) {
                Ok((shortcut, warning)) => {
                    if warning.is_some() {
                        message = warning;
                    }
                    Some(shortcut)
                }
                Err(error) => {
                    message = Some(Text::GlobalShortcutError.context(error));
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
        let layers: BTreeMap<_, _> = displays
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
        let tray = if demo {
            None
        } else {
            let sender = sender.clone();
            let ctx = cc.egui_ctx.clone();
            match crate::tray::Tray::new(locale, move |action| {
                let _ = sender.send(Event::Tray(action));
                ctx.request_repaint_of(ViewportId::ROOT);
            }) {
                Ok(tray) => Some(tray),
                Err(error) => {
                    message = Some(Text::TrayError.context(error));
                    cc.egui_ctx
                        .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
                    cc.egui_ctx
                        .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Decorations(true));
                    cc.egui_ctx.send_viewport_cmd_to(
                        ViewportId::ROOT,
                        ViewportCommand::MousePassthrough(false),
                    );
                    None
                }
            }
        };
        let (startup, startup_enabled) = if demo {
            (None, false)
        } else {
            match crate::startup::Startup::new()
                .and_then(|startup| startup.enabled().map(|enabled| (startup, enabled)))
            {
                Ok((startup, enabled)) => (Some(startup), enabled),
                Err(error) => {
                    message = Some(Text::StartupError.context(error));
                    (None, false)
                }
            }
        };
        #[cfg(target_os = "macos")]
        let dock = if demo {
            None
        } else {
            let sender = sender.clone();
            let ctx = cc.egui_ctx.clone();
            match crate::macos_dock::Dock::new(move |action| {
                let _ = sender.send(Event::Tray(action));
                ctx.request_repaint_of(ViewportId::ROOT);
            }) {
                Ok(mut dock) => {
                    if let Err(error) = dock.load_menu() {
                        message = Some(error);
                    }
                    Some(dock)
                }
                Err(error) => {
                    message = Some(error);
                    None
                }
            }
        };
        let home_visible = !std::env::args().any(|arg| arg == "--background")
            || tray.is_none()
            || layers.is_empty();
        #[cfg(target_os = "macos")]
        let pin_drag = if demo {
            platform::macos_drag::DragEvents::default()
        } else {
            platform::macos_drag::DragEvents::install().unwrap_or_else(|error| {
                message = Some(error);
                platform::macos_drag::DragEvents::default()
            })
        };
        Self {
            events,
            sender,
            phase: Phase::Idle,
            gate: CaptureGate::default(),
            capture_worker: None,
            layers,
            active_display: None,
            pins: BTreeMap::new(),
            pin_menu: None,
            unlock_hint: None,
            quit_confirmation: None,
            next_quit_request: 0,
            #[cfg(target_os = "macos")]
            pin_drag,
            next_pin: 0,
            message,
            system_locale,
            shortcut,
            preferences,
            applied_icon: None,
            icon_data: None,
            icon_texture: None,
            config_path,
            recording: false,
            suppress_shortcut_until: Instant::now(),
            demo,
            tray,
            #[cfg(target_os = "macos")]
            dock,
            startup,
            startup_enabled,
            exiting: false,
            home_visible,
            booting: true,
            permission: ScreenPermission::default(),
            relaunch,
        }
    }

    fn capture_command(&mut self, ctx: &egui::Context) {
        if self.exiting || self.quit_confirmation.is_some() {
            return;
        }
        if matches!(self.phase, Phase::Capturing) {
            self.reset_capture(ctx);
        } else {
            self.begin_capture(ctx);
        }
    }

    fn begin_capture(&mut self, ctx: &egui::Context) {
        for id in self.pins.keys() {
            ui::pin::forget_input(ctx, *id);
        }
        if !matches!(self.phase, Phase::Idle) || self.recording {
            return;
        }
        self.clear_unlock_hint(ctx);
        if !self.check_capture_permission(
            ctx,
            platform::capture_permission,
            platform::request_capture_permission,
        ) {
            return;
        }
        let Some(id) = self.gate.begin() else {
            return;
        };
        self.close_pin_menu(ctx);
        self.message = None;
        self.phase = Phase::Capturing;
        // Preserve the visible desktop while capturing. No hiding, animation,
        // fullscreen transition or artificial delay is needed.
        let demo_origin = ctx.input(|input| {
            input
                .viewport()
                .outer_rect
                .map(|rect| [rect.min.x, rect.min.y])
        });
        if self.capture_worker.is_none() {
            let sender = self.sender.clone();
            let repaint = ctx.clone();
            let demo = self.demo;
            match capture::Worker::new(
                move |origin, cancelled| {
                    if demo {
                        Ok(vec![capture::demo_windows(origin)])
                    } else {
                        capture::desktop(cancelled)
                    }
                },
                move |id, result| {
                    let pointer = (!demo && result.is_ok())
                        .then(platform::pointer_position)
                        .flatten();
                    let _ = sender.send(Event::Ready {
                        request_id: id,
                        result,
                        pointer,
                    });
                    repaint.request_repaint_of(ViewportId::ROOT);
                },
            ) {
                Ok(worker) => self.capture_worker = Some(worker),
                Err(error) => {
                    self.message = Some(error);
                    self.reset_capture(ctx);
                    return;
                }
            }
        }
        if let Some(worker) = &self.capture_worker
            && let Err(error) = worker.request(id, demo_origin)
        {
            self.message = Some(error);
            self.reset_capture(ctx);
        }
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn check_capture_permission(
        &mut self,
        ctx: &egui::Context,
        preflight: impl FnMut() -> bool,
        request: impl FnOnce() -> bool,
    ) -> bool {
        if self.demo || self.permission.authorize(preflight, request) {
            return true;
        }
        self.message = None;
        self.tray_action(crate::tray::Action::Home, ctx);
        false
    }

    fn restart(&mut self, ctx: &egui::Context) {
        if self.demo {
            return;
        }
        match platform::relaunch_command() {
            Ok(command) => {
                if let Ok(mut relaunch) = self.relaunch.lock() {
                    *relaunch = Some(command);
                } else {
                    self.message = Some(Text::RestartError.into());
                    return;
                }
                self.tray_action(crate::tray::Action::Quit, ctx);
            }
            Err(error) => self.message = Some(error),
        }
    }

    fn handle_event(&mut self, event: Event, ctx: &egui::Context) {
        match event {
            Event::HintError(error) => self.message = Some(Text::ScreenHintError.context(error)),
            Event::UnlockHint(id) => {
                let point = if self.demo {
                    None
                } else {
                    platform::pointer_position()
                };
                self.show_unlock_hint(id, point, ctx);
            }
            Event::Capture => {
                if Instant::now() >= self.suppress_shortcut_until {
                    self.capture_command(ctx);
                }
            }
            Event::Ready {
                request_id: id,
                result,
                pointer,
            } => self.accept_capture(id, result, pointer, ctx),
            Event::Selection {
                request_id: id,
                display_id,
                action,
            } => self.selection_action(id, display_id, action, ctx),
            Event::PinFrameDrawn {
                session,
                pin_id: pin,
            } => {
                if matches!(&self.phase, Phase::Selecting { id, pending_pin: Some(pending), .. } if *id == session && *pending == pin)
                {
                    self.reset_capture(ctx);
                }
            }
            #[cfg(target_os = "macos")]
            Event::BeginPinDrag { pin_id: id, press } => {
                if self.pins.get(&id).is_some_and(|pin| !pin.locked) {
                    self.pin_drag.begin(id, press);
                }
            }
            Event::CopyPin(id) => {
                self.copy_pin(id, ctx);
                self.close_pin_menu(ctx);
            }
            Event::ClosePin(id) => self.close_pin(id, ctx),
            Event::ToggleLock(id) => self.toggle_pin_lock(id, ctx),
            Event::ZoomPin {
                pin_id: id,
                commands,
                observed,
            } => self.zoom_pin(id, &commands, observed, ctx),
            Event::MenuPin {
                pin_id: id,
                position: anchor,
            } => self.open_pin_menu(id, anchor, ctx),
            Event::MovePin {
                pin_id: id,
                steps,
                observed,
            } => self.move_pin(id, steps, observed, ctx),
            Event::ClosePinMenu => self.close_pin_menu(ctx),
            Event::Tray(action) => self.tray_action(action, ctx),
            Event::QuitDecision {
                request_id: id,
                outcome,
            } => self.quit_decision(id, outcome, ctx),
        }
    }

    fn tray_action(&mut self, action: crate::tray::Action, ctx: &egui::Context) {
        use crate::tray::Action;
        match action {
            Action::Home => {
                self.home_visible = true;
                // Only a pending confirmation needs to stay above locked pins.
                ctx.send_viewport_cmd_to(
                    ViewportId::ROOT,
                    ViewportCommand::WindowLevel(if self.quit_confirmation.is_some() {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    }),
                );
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Decorations(true));
                ctx.send_viewport_cmd_to(
                    ViewportId::ROOT,
                    ViewportCommand::MousePassthrough(false),
                );
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Minimized(false));
                // Visible(true) can make Home key before egui-winit handles
                // Focus, causing its key-window guard to skip app activation.
                #[cfg(target_os = "macos")]
                if !self.demo
                    && let Err(error) = platform::activate_application()
                {
                    self.message = Some(error);
                }
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
            }
            Action::Capture => self.capture_command(ctx),
            Action::UnlockPins => self.unlock_pins(ctx),
            Action::Pin { id, command } => {
                self.handle_event(
                    pin_event(ui::pin::Action::from_menu(id, command), ctx, None),
                    ctx,
                );
            }
            Action::ClosePins => {
                self.close_pins(ctx);
                if matches!(
                    self.phase,
                    Phase::Selecting {
                        pending_pin: Some(_),
                        ..
                    }
                ) {
                    self.reset_capture(ctx);
                }
            }
            Action::Quit => {
                if self.exiting {
                    #[cfg(target_os = "macos")]
                    if let Some(dock) = &self.dock {
                        dock.resolve_quit(true);
                    }
                    return;
                }
                if self.quit_confirmation.is_some() {
                    return;
                }
                self.recording = false;
                self.reset_capture(ctx);
                if self.pins.values().any(|pin| pin.locked) {
                    self.next_quit_request = self.next_quit_request.wrapping_add(1);
                    self.quit_confirmation = Some(QuitConfirmation {
                        id: self.next_quit_request,
                        presentation: if self.demo {
                            QuitPresentation::Fallback
                        } else {
                            QuitPresentation::AwaitingHost
                        },
                    });
                    self.tray_action(Action::Home, ctx);
                } else {
                    self.quit(ctx);
                }
            }
        }
    }

    fn set_startup(&mut self, enabled: bool) {
        if let Some(startup) = &self.startup {
            match startup.set_enabled(enabled) {
                Ok(enabled) => {
                    self.startup_enabled = enabled;
                    self.message = None;
                }
                Err(error) => self.message = Some(error),
            }
        }
    }

    fn prepare_home(&mut self, ctx: &egui::Context) -> bool {
        if !self.home_visible {
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
            if self.booting {
                self.booting = false;
                ctx.request_repaint();
            }
            false
        } else {
            if self.booting {
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Decorations(true));
                ctx.send_viewport_cmd_to(
                    ViewportId::ROOT,
                    ViewportCommand::MousePassthrough(false),
                );
            }
            self.booting = false;
            true
        }
    }

    fn cancel_capture(&mut self, ctx: &egui::Context) {
        if let Phase::Selecting { pending_pin, .. } = &mut self.phase
            && let Some(id) = pending_pin.take()
        {
            self.close_pin(id, ctx);
        }
        self.reset_capture(ctx);
    }

    fn reset_capture(&mut self, ctx: &egui::Context) {
        ctx.set_cursor_icon(egui::CursorIcon::Default);
        self.gate.cancel();
        if let Some(worker) = &self.capture_worker {
            worker.cancel();
        }
        let was_selecting = matches!(self.phase, Phase::Selecting { .. });
        for (&display_id, layer) in &mut self.layers {
            layer.selection = None;
            ctx.request_repaint_of(layer_id(display_id));
        }
        self.phase = Phase::Idle;
        self.active_display = None;
        if was_selecting && !self.demo {
            platform::finish_capture(ctx);
        }
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn accept_capture(
        &mut self,
        id: u64,
        result: Result<Vec<Snapshot>, Message>,
        pointer: Option<[f32; 2]>,
        ctx: &egui::Context,
    ) {
        if !self.gate.complete(id) {
            return;
        }
        let result = result.and_then(|snapshots| {
            let limit = ctx.input(|input| input.max_texture_side);
            if snapshots
                .iter()
                .any(|snapshot| snapshot.pixels.size.iter().any(|side| *side > limit))
            {
                Err(Text::TextureLimit.with(limit))
            } else {
                Ok(snapshots)
            }
        });
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
                        let texture = ctx.load_texture(
                            format!("selection-{}", display.id),
                            snapshot.pixels.clone(),
                            egui::TextureOptions::LINEAR,
                        );
                        let selection = overlay::Selection {
                            texture,
                            pixels: snapshot.pixel_size(),
                            pixel_step: platform::selection_pixel_step(snapshot),
                            windows: snapshot
                                .window_regions(platform::selection_pixel_step(snapshot)),
                            press: None,
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
                self.message = Some(result.err().unwrap_or_else(|| Text::NoDisplays.into()));
                if !self.demo && !platform::capture_permission() {
                    self.permission = ScreenPermission::Required;
                }
                self.reset_capture(ctx);
                self.tray_action(crate::tray::Action::Home, ctx);
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
            self.cancel_capture(ctx);
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
                snapshot.pixel_size(),
                snapshot.display.position(),
                snapshot.display.logical_size(),
            );
            let pixels = Arc::new(pixels);
            let texture = ctx.load_texture(
                format!("pin-{}", self.next_pin),
                pixels.clone(),
                egui::TextureOptions::LINEAR,
            );
            ctx.copy_image((*pixels).clone());
            let displays: Vec<_> = self
                .layers
                .values()
                .map(|layer| layer.display.clone())
                .collect();
            let areas = display_areas(&displays, self.demo);
            let screen = displays
                .iter()
                .zip(&areas)
                .find(|(display, _)| display.id == display_id)
                .map(|(_, area)| *area)
                .unwrap_or_else(|| {
                    // A source display must still get its own current work area
                    // if its warmed layer is no longer in the topology snapshot.
                    display_areas(std::slice::from_ref(&snapshot.display), self.demo)[0]
                });
            let screen_areas = Arc::new(
                displays
                    .iter()
                    .zip(areas)
                    .map(|(display, area)| {
                        // Normalize mixed-DPI bounds into the capture's coordinates.
                        let factor = if cfg!(target_os = "macos") {
                            1.0
                        } else {
                            display.scale / snapshot.display.scale
                        };
                        egui::Rect::from_min_max(area.min * factor, area.max * factor)
                    })
                    .collect(),
            );
            self.pins.insert(
                self.next_pin,
                Pin {
                    name: String::new(),
                    zoom: platform::pin_zoom::PinZoom::new(self.next_pin),
                    texture,
                    pixels,
                    initial: {
                        let zoom = ctx.zoom_factor();
                        let ratio = pin::coordinate_ratio(
                            snapshot.display.scale,
                            zoom,
                            snapshot.display.scale * zoom,
                        );
                        pin::frame_geometry(placement, screen, ratio).image
                    },
                    display_scale: snapshot.display.scale,
                    screen,
                    screen_areas,
                    session: id,
                    first_frame_drawn: Arc::new(AtomicBool::new(false)),
                    locked: false,
                    #[cfg(target_os = "macos")]
                    lock_limits: None,
                    #[cfg(target_os = "macos")]
                    show_requested: Cell::new(false),
                },
            );
            // Keep the selection image until the pin has painted its first frame.
            *pending_pin = Some(self.next_pin);
            ctx.request_repaint_of(ViewportId::ROOT);
        } else {
            self.reset_capture(ctx);
        }
    }

    fn copy_pin(&self, id: u64, ctx: &egui::Context) {
        if let Some(pin) = self.pins.get(&id) {
            ctx.copy_image((*pin.pixels).clone());
        }
    }

    fn open_pin_menu(&mut self, id: u64, position: Option<egui::Pos2>, ctx: &egui::Context) {
        ui::pin::forget_input(ctx, id);
        let Some(pin) = self.pins.get(&id) else {
            return;
        };
        if !self.demo {
            match platform::pin_menu::show(
                id,
                self.preferences.language.resolve(self.system_locale),
                pin.locked,
            ) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => {
                    self.message = Some(error);
                    self.tray_action(crate::tray::Action::Home, ctx);
                    return;
                }
            }
        }
        self.pin_menu = Some(PinMenu {
            pin_id: id,
            position,
            focused: Arc::new(AtomicBool::new(false)),
        });
        ctx.request_repaint_of(pin_menu_id());
    }

    fn close_pin_menu(&mut self, ctx: &egui::Context) {
        if self.pin_menu.take().is_some() {
            ctx.send_viewport_cmd_to(pin_menu_id(), ViewportCommand::Close);
        }
    }

    fn close_pin(&mut self, id: u64, ctx: &egui::Context) {
        if self.pins.get(&id).is_some_and(|pin| pin.locked) {
            return;
        }
        if self.pin_menu.as_ref().is_some_and(|menu| menu.pin_id == id) {
            self.close_pin_menu(ctx);
        }
        if let Some(pin) = self.pins.remove(&id) {
            pin.release(id, ctx);
        }
        if matches!(&self.phase, Phase::Selecting { pending_pin: Some(pending), .. } if *pending == id)
        {
            self.reset_capture(ctx);
        }
    }

    fn close_pins(&mut self, ctx: &egui::Context) {
        self.close_pin_menu(ctx);
        for id in self.pins.keys().copied().collect::<Vec<_>>() {
            self.close_pin(id, ctx);
        }
    }

    fn toggle_pin_lock(&mut self, id: u64, ctx: &egui::Context) {
        if let Some(pin) = self.pins.get(&id) {
            self.set_pin_locked(id, !pin.locked, ctx);
        }
    }

    fn unlock_pins(&mut self, ctx: &egui::Context) {
        let ids: Vec<_> = self
            .pins
            .iter()
            .filter_map(|(&id, pin)| pin.locked.then_some(id))
            .collect();
        for id in ids {
            self.set_pin_locked(id, false, ctx);
        }
    }

    fn clear_unlock_hint(&mut self, ctx: &egui::Context) {
        if let Some(hint) = self.unlock_hint.take() {
            for &display in hint.areas.keys() {
                ctx.request_repaint_of(layer_id(display));
            }
        }
    }

    fn show_unlock_hint(&mut self, id: u64, point: Option<[f32; 2]>, ctx: &egui::Context) {
        if !matches!(self.phase, Phase::Idle) || !self.pins.get(&id).is_some_and(|pin| pin.locked) {
            return;
        }
        self.clear_unlock_hint(ctx);
        // Missing desktop coordinates (for example Wayland) must not make a
        // recovery notice unreachable. Use all known display layers there.
        let display = point.and_then(|point| {
            self.layers
                .iter()
                .find(|(_, layer)| layer.display.contains(point))
                .map(|(&id, _)| id)
        });
        let displays: Vec<_> = self
            .layers
            .values()
            .filter(|layer| display.is_none_or(|id| layer.display.id == id))
            .map(|layer| layer.display.clone())
            .collect();
        let areas = display_areas(&displays, self.demo);
        for display in &displays {
            ctx.request_repaint_of(layer_id(display.id));
        }
        self.unlock_hint = Some(UnlockHint {
            pin: id,
            areas: displays
                .iter()
                .map(|display| display.id)
                .zip(areas)
                .collect(),
            until: Instant::now() + ui::screen_hint::UNLOCK_DURATION,
            raised: Arc::new(AtomicBool::new(false)),
        });
        ctx.request_repaint_after_for(ui::screen_hint::UNLOCK_DURATION, ViewportId::ROOT);
    }

    fn set_pin_locked(&mut self, id: u64, locked: bool, ctx: &egui::Context) {
        ui::pin::forget_input(ctx, id);
        let Some(pin) = self.pins.get_mut(&id) else {
            return;
        };
        #[cfg(target_os = "macos")]
        if !self.demo
            && let Err(error) = platform::set_pin_locked(id, locked, &mut pin.lock_limits)
        {
            self.message = Some(error);
            return;
        }
        pin.locked = locked;
        #[cfg(not(target_os = "macos"))]
        ctx.send_viewport_cmd_to(pin_id(id), ViewportCommand::Resizable(!locked));
        if !locked && self.unlock_hint.as_ref().is_some_and(|hint| hint.pin == id) {
            self.clear_unlock_hint(ctx);
        }
        self.close_pin_menu(ctx);
        ctx.request_repaint_of(pin_id(id));
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn select_pin(&mut self, id: u64, ctx: &egui::Context) {
        if !self.pins.contains_key(&id) {
            return;
        }
        if !self.demo
            && let Err(error) = platform::pin_window::activate(id)
        {
            self.message = Some(error);
        }
        ctx.request_repaint_of(pin_id(id));
    }

    fn zoom_pin(
        &mut self,
        id: u64,
        commands: &[pin::Zoom],
        observed: Option<ObservedSize>,
        ctx: &egui::Context,
    ) {
        let Some(pin) = self
            .pins
            .get(&id)
            .filter(|pin| !pin.locked && !commands.is_empty())
        else {
            return;
        };
        let pixels = pin.pixels.size;
        let zoom = ctx.zoom_factor();
        let geometry = observed.or_else(|| {
            ctx.input(|input| {
                let viewport = input.raw.viewports.get(&pin_id(id))?;
                let scale = viewport.native_pixels_per_point?;
                let current = match viewport.inner_rect {
                    Some(rect) => rect.size() * zoom,
                    // Restoring source size does not need a global position.
                    None if commands.first() == Some(&pin::Zoom::Original) => {
                        egui::vec2(pixels[0] as f32, pixels[1] as f32) / scale
                    }
                    None => return None,
                };
                Some(ObservedSize {
                    content_size: current,
                    native_pixels_per_point: scale,
                })
            })
        });
        if !self.demo
            && let Some(ObservedSize {
                native_pixels_per_point: scale,
                ..
            }) = geometry
        {
            let Some(pin) = self.pins.get_mut(&id) else {
                return;
            };
            match pin.zoom.apply(pixels, scale, commands) {
                Ok(platform::pin_zoom::Outcome::Applied) => {
                    self.close_pin_menu(ctx);
                    return;
                }
                Ok(platform::pin_zoom::Outcome::SizeOnly) => {}
                Err(error) => {
                    self.message = Some(error);
                    return;
                }
            }
        }
        if let Some(ObservedSize {
            content_size: current,
            native_pixels_per_point: scale,
        }) = geometry
            && let Some(mut size) = commands.iter().try_fold(current, |size, &command| {
                pin::zoom_size(size, pixels, scale, command)
            })
        {
            if self.demo
                && commands
                    .iter()
                    .any(|command| matches!(command, pin::Zoom::At { .. }))
                && let Some(position) = ctx.input(|input| {
                    input
                        .raw
                        .viewports
                        .get(&pin_id(id))
                        .and_then(|viewport| viewport.inner_rect)
                        .map(|rect| rect.min * zoom)
                })
                && let Some(target) = pin::zoom_rect(
                    egui::Rect::from_min_size(position, current),
                    pixels,
                    scale,
                    commands,
                )
            {
                ctx.send_viewport_cmd_to(
                    pin_id(id),
                    ViewportCommand::OuterPosition(target.min / zoom),
                );
                size = target.size();
            }
            ctx.send_viewport_cmd_to(pin_id(id), ViewportCommand::InnerSize(size / zoom));
        }
        self.close_pin_menu(ctx);
    }

    fn move_pin(
        &self,
        id: u64,
        steps: [i32; 2],
        observed: Option<ObservedPosition>,
        ctx: &egui::Context,
    ) {
        if !self.pins.get(&id).is_some_and(|pin| !pin.locked) {
            return;
        }
        let zoom = ctx.zoom_factor();
        let geometry = observed.or_else(|| {
            ctx.input(|input| {
                let viewport = input.raw.viewports.get(&pin_id(id))?;
                Some(ObservedPosition {
                    outer_origin: viewport.outer_rect?.min * zoom,
                    native_pixels_per_point: viewport.native_pixels_per_point?,
                })
            })
        });
        if let Some(ObservedPosition {
            outer_origin: position,
            native_pixels_per_point: scale,
        }) = geometry
            && position.is_finite()
            && let Some(delta) = pin::movement_delta(steps, scale)
            && (position + delta).is_finite()
        {
            ctx.send_viewport_cmd_to(
                pin_id(id),
                ViewportCommand::OuterPosition((position + delta) / zoom),
            );
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.exiting = true;
        let _ = self.quit_confirmation.take();
        #[cfg(target_os = "macos")]
        if let Some(dock) = &self.dock {
            dock.resolve_quit(true);
        }
        self.close_pin_menu(ctx);
        for (id, pin) in std::mem::take(&mut self.pins) {
            pin.release(id, ctx);
        }
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
    }

    fn cancel_quit(&mut self, ctx: &egui::Context) {
        // Invalidate the request before dropping a sheet: dismissal may deliver
        // its completion synchronously, but that queued result is now stale.
        let _ = self.quit_confirmation.take();
        #[cfg(target_os = "macos")]
        if let Some(dock) = &self.dock {
            dock.resolve_quit(false);
        }
        ctx.send_viewport_cmd_to(
            ViewportId::ROOT,
            ViewportCommand::WindowLevel(egui::WindowLevel::Normal),
        );
        if let Ok(mut command) = self.relaunch.lock() {
            command.take();
        }
    }

    fn menu_state(&self) -> crate::tray::MenuState {
        crate::tray::MenuState {
            locale: self.preferences.language.resolve(self.system_locale),
            capture_enabled: matches!(self.phase, Phase::Idle | Phase::Capturing)
                && !self.recording
                && self.quit_confirmation.is_none(),
            has_unlocked_pins: self.pins.values().any(|pin| !pin.locked),
            has_locked_pins: self.pins.values().any(|pin| pin.locked),
            capturing: matches!(self.phase, Phase::Capturing),
        }
    }

    fn close_home(&mut self, ctx: &egui::Context) {
        if self.exiting {
            return;
        }
        if self.quit_confirmation.as_ref().is_some_and(|confirmation| {
            matches!(&confirmation.presentation, QuitPresentation::Native(dialog) if !dialog.can_dismiss())
        }) {
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::CancelClose);
            return;
        }
        if self.tray.is_some() {
            self.cancel_quit(ctx);
            self.home_visible = false;
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::CancelClose);
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
        } else {
            if self.quit_confirmation.is_some() || self.pins.values().any(|pin| pin.locked) {
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::CancelClose);
            }
            self.tray_action(crate::tray::Action::Quit, ctx);
        }
    }

    fn quit_decision(&mut self, id: u64, result: Result<bool, Message>, ctx: &egui::Context) {
        let Some(pending) = &mut self.quit_confirmation else {
            return;
        };
        if pending.id != id {
            return;
        }
        match result {
            Ok(true) => self.quit(ctx),
            Ok(false) => self.cancel_quit(ctx),
            Err(error) => {
                pending.presentation = QuitPresentation::Fallback;
                self.message = Some(error);
                ctx.request_repaint_of(ViewportId::ROOT);
            }
        }
    }

    fn prepare_quit_prompt(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(confirmation) = &mut self.quit_confirmation else {
            return;
        };
        match confirmation.presentation {
            QuitPresentation::AwaitingHost => {
                // Let Home's visibility/focus commands reach the native host
                // before attaching a sheet. This is a single event-driven step.
                confirmation.presentation = QuitPresentation::Ready;
                ctx.request_repaint_of(ViewportId::ROOT);
                return;
            }
            QuitPresentation::Ready => {
                // A discarded egui pass can call logic again in the same native
                // frame. Start only on a later frame's first pass.
                if ctx.current_pass_index() != 0 {
                    ctx.request_repaint_of(ViewportId::ROOT);
                    return;
                }
            }
            _ => return,
        }
        let id = confirmation.id;
        let sender = self.sender.clone();
        let wake = ctx.clone();
        let result = move |outcome| {
            let _ = sender.send(Event::QuitDecision {
                request_id: id,
                outcome,
            });
            wake.request_repaint_of(ViewportId::ROOT);
        };
        let opened = match frame.window_handle() {
            Ok(owner) => platform::quit_prompt::show(
                owner,
                self.preferences.language.resolve(self.system_locale),
                result,
            ),
            Err(_) => Ok(None),
        };
        confirmation.presentation = match opened {
            Ok(Some(dialog)) => QuitPresentation::Native(dialog),
            Ok(None) => QuitPresentation::Fallback,
            Err(error) => {
                self.message = Some(error);
                QuitPresentation::Fallback
            }
        };
    }

    fn change_binding(&mut self, binding: Binding, ctx: &egui::Context) {
        if let Err(error) = binding.validate_capture() {
            self.message = Some(error);
            return;
        }
        self.suppress_shortcut_until = Instant::now() + SHORTCUT_SUPPRESSION;
        let mut registration_warning = None;
        if self.shortcut.is_none() && !self.demo && platform::global_shortcuts_supported() {
            match register_shortcut(binding, &self.sender, ctx) {
                Ok((shortcut, warning)) => {
                    self.shortcut = Some(shortcut);
                    registration_warning = warning;
                }
                Err(error) => {
                    self.message = Some(Text::ShortcutError.context(error));
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
        self.message = registration_warning.or_else(|| {
            Some(
                if self
                    .shortcut
                    .as_ref()
                    .is_some_and(Shortcut::capture_registered)
                {
                    Text::ShortcutSaved
                } else {
                    Text::ShortcutSavedLocal
                }
                .into(),
            )
        });
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
        self.applied_icon = None;
        ctx.set_theme(theme.preference());
        self.message = Some(
            if self.demo {
                Text::DemoAppearance
            } else {
                Text::AppearanceSaved
            }
            .into(),
        );
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    fn change_language(&mut self, language: Language, ctx: &egui::Context) {
        let preferences = Preferences {
            language,
            ..self.preferences
        };
        if let Some(path) = &self.config_path
            && let Err(error) = preferences.save(path)
        {
            self.message = Some(error);
            return;
        }
        self.preferences = preferences;
        i18n::set_locale(ctx, language.resolve(self.system_locale));
        ctx.send_viewport_cmd_to(
            ViewportId::ROOT,
            ViewportCommand::Title(
                language
                    .resolve(self.system_locale)
                    .text(Text::WindowTitle)
                    .into(),
            ),
        );
        ctx.request_repaint_of(ViewportId::ROOT);
        for id in self.pins.keys() {
            ctx.request_repaint_of(pin_id(*id));
        }
        if self.pin_menu.is_some() {
            ctx.request_repaint_of(pin_menu_id());
        }
        if self.unlock_hint.is_some() {
            for &display in self.layers.keys() {
                ctx.request_repaint_of(layer_id(display));
            }
        }
    }

    fn update_icon(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "macos")]
        if self.preferences.icon == settings::Icon::Auto
            && self.preferences.theme == settings::Theme::System
            && ctx.system_theme().is_none()
        {
            // A logic-only startup pass has no theme yet. Keep the bundle icon
            // until the normal theme event supplies an authoritative choice.
            return;
        }
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
        match ui.input(|input| ui::shortcut::record(&input.events)) {
            Some(ui::shortcut::Recording::Cancel) => self.recording = false,
            Some(ui::shortcut::Recording::Binding(binding)) => {
                self.recording = false;
                self.change_binding(binding, ui.ctx());
            }
            Some(ui::shortcut::Recording::Invalid(error)) => self.message = Some(error),
            None => {}
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
            let hint_raised = self.unlock_hint.as_ref().map(|hint| hint.raised.clone());
            let events = self.sender.clone();
            let active = selection.is_some();
            let pointer_display = self.active_display == Some(display_id);
            let display = layer.display.clone();
            let hint_area = self
                .unlock_hint
                .as_ref()
                .and_then(|hint| hint.areas.get(&display_id).copied());
            let position = display.position();
            let size = display.logical_size();
            ctx.show_viewport_deferred(
                layer_id(display_id),
                self.window_icon(egui::ViewportBuilder::default())
                    .with_title(snip2pin::SELECTION_WINDOW_TITLE)
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
                    // Root logic reports any pin-adapter failure.
                    let _ = platform::prepare_floating_windows();
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
                    if !active {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                        if let Some(area) = hint_area {
                            if !demo
                                && hint_raised
                                    .as_ref()
                                    .is_some_and(|raised| !raised.swap(true, Ordering::AcqRel))
                            {
                                let result = platform::raise_screen_hint();
                                if let Err(error) = result {
                                    let _ = events.send(Event::HintError(error));
                                    ui.ctx().request_repaint_of(ViewportId::ROOT);
                                }
                            }
                            let ratio = pin::coordinate_ratio(
                                display.scale,
                                ui.ctx().zoom_factor(),
                                ui.ctx().pixels_per_point(),
                            );
                            let origin = egui::Vec2::from(display.position());
                            let local = egui::Rect::from_min_max(
                                (area.min - origin) * ratio,
                                (area.max - origin) * ratio,
                            )
                            .intersect(ui.max_rect());
                            ui::screen_hint::show(
                                ui,
                                local,
                                &ui::screen_hint::unlock_text(ui.ctx()),
                            );
                        }
                    }
                    if let (Some(selection), Some(id)) = (&selection, session)
                        && let Ok(mut selection) = selection.lock()
                        && let Some(action) = overlay::show(ui, &mut selection, pointer_display)
                    {
                        let _ = events.send(Event::Selection {
                            request_id: id,
                            display_id,
                            action,
                        });
                        ui.ctx().request_repaint_of(ViewportId::ROOT);
                    }
                },
            );
        }
    }

    fn show_pins(&self, ctx: &egui::Context) {
        let fallback_menu = self.demo || cfg!(target_os = "linux");
        for (&id, pin) in &self.pins {
            let texture = pin.texture.clone();
            let events = self.sender.clone();
            let first_frame_drawn = pin.first_frame_drawn.clone();
            let session = pin.session;
            let display_scale = pin.display_scale;
            let screen = pin.screen;
            let areas = pin.screen_areas.clone();
            let initial = pin.initial;
            let locked = pin.locked;
            let demo = self.demo;
            #[cfg(target_os = "macos")]
            let native_drag = (!self.demo).then(|| self.pin_drag.requests());
            let builder = self
                .window_icon(egui::ViewportBuilder::default())
                .with_title(platform::pin_window::title(id))
                .with_decorations(false)
                .with_transparent(false)
                .with_has_shadow(true)
                .with_position(initial.min)
                .with_inner_size(initial.size())
                .with_resizable(cfg!(target_os = "macos") || !locked)
                .with_always_on_top()
                .with_active(false)
                .with_taskbar(false);
            #[cfg(target_os = "macos")]
            let builder = {
                // Querying maximized state moves an ordered borderless AppKit window.
                // Native geometry confirms hidden backend initialization, even when
                // occlusion prevents the image callback from painting its first frame.
                let initialized = ctx.input(|input| {
                    input
                        .raw
                        .viewports
                        .get(&pin_id(id))
                        .is_some_and(|viewport| viewport.inner_rect.is_some())
                });
                if initialized && !pin.show_requested.replace(true) {
                    ctx.send_viewport_cmd_to(pin_id(id), ViewportCommand::Visible(true));
                }
                builder.with_visible(false)
            };
            ctx.show_viewport_deferred(pin_id(id), builder, move |ui, _| {
                let ratio = pin::coordinate_ratio(
                    display_scale,
                    ui.ctx().zoom_factor(),
                    ui.ctx().pixels_per_point(),
                );
                // Native geometry is authoritative; painting never changes it.
                let controls = if let Some(image) = ui.input(|input| input.viewport().inner_rect) {
                    (if demo {
                        pin::toolbar_geometry(image, &areas, screen, ratio)
                    } else {
                        platform::pin_toolbar(id, image, &areas, screen, ratio)
                    })
                    .map(|toolbar| toolbar.translate(-image.min.to_vec2()))
                } else {
                    // Wayland may not expose global positions. Keep controls
                    // inside the observed content instead of guessing one.
                    let bounds = ui.max_rect();
                    pin::toolbar_geometry(bounds, &[], bounds, 1.0)
                };
                let ctx = ui.ctx().clone();
                let menu_position = fallback_menu
                    .then(|| {
                        ctx.input(|input| {
                            input
                                .viewport()
                                .inner_rect
                                .zip(input.pointer.interact_pos())
                                .map(|(window, point)| {
                                    pin::menu_position(
                                        window.min + point.to_vec2(),
                                        ui::pin::MENU_SIZE,
                                        &areas,
                                        screen,
                                        ratio,
                                    )
                                })
                        })
                    })
                    .flatten();
                let dispatch = |action| dispatch_pin(action, &events, &ctx, menu_position);
                ui::pin::route_gesture(ui, &dispatch);
                #[cfg(target_os = "macos")]
                let press_origin = ui.input(|input| input.pointer.press_origin());
                ui::pin::show(
                    ui,
                    id,
                    &texture,
                    ui::pin::Interaction {
                        native_edges: cfg!(target_os = "macos"),
                        locked,
                        toolbar: controls,
                        keyboard_focus: None,
                        space: platform::pin_input_space(ui),
                    },
                    &dispatch,
                    |command| {
                        #[cfg(target_os = "macos")]
                        if command == ViewportCommand::StartDrag
                            && let Some(requests) = &native_drag
                        {
                            if ctx.current_pass_index() == 0
                                && let Some(origin) = press_origin
                                && let Some(press) =
                                    requests.matching(id, origin, ctx.pixels_per_point())
                            {
                                let _ = events.send(Event::BeginPinDrag { pin_id: id, press });
                                ctx.request_repaint_of(ViewportId::ROOT);
                            }
                            return;
                        }
                        ctx.send_viewport_cmd(command);
                    },
                );
                let small = launcher::pin_is_small(ui.max_rect().size());
                let changed = ctx.data_mut(|data| {
                    let key = pin_hint_state_id(id);
                    let previous = data.get_temp::<bool>(key);
                    data.insert_temp(key, small);
                    previous != Some(small)
                });
                if changed {
                    ctx.request_repaint_of(ViewportId::ROOT);
                }
                if ui.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, egui::Event::WindowFocused(_)))
                }) {
                    ctx.request_repaint_of(ViewportId::ROOT);
                }
                if !first_frame_drawn.swap(true, Ordering::AcqRel) {
                    let _ = events.send(Event::PinFrameDrawn {
                        session,
                        pin_id: id,
                    });
                    ui.ctx().request_repaint_of(ViewportId::ROOT);
                }
            });
        }
    }

    fn show_pin_menu(&self, ctx: &egui::Context) {
        let Some(menu) = &self.pin_menu else {
            return;
        };
        let id = menu.pin_id;
        let locked = self.pins.get(&id).is_some_and(|pin| pin.locked);
        let focused = menu.focused.clone();
        let events = self.sender.clone();
        let mut builder = self
            .window_icon(egui::ViewportBuilder::default())
            .with_title("Snip2Pin · 操作")
            .with_decorations(false)
            .with_transparent(false)
            .with_inner_size(ui::pin::MENU_SIZE)
            .with_resizable(false)
            .with_has_shadow(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_active(true);
        if let Some(position) = menu.position {
            builder = builder.with_position(position);
        }
        ctx.show_viewport_deferred(pin_menu_id(), builder, move |ui, _| {
            let has_focus = ui.input(|input| input.focused);
            if has_focus {
                focused.store(true, Ordering::Release);
            }
            if ui.input(|input| {
                input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
            }) || (!has_focus && focused.load(Ordering::Acquire))
            {
                let _ = events.send(Event::ClosePinMenu);
                ui.ctx().request_repaint_of(ViewportId::ROOT);
                return;
            }
            let ctx = ui.ctx().clone();
            ui::pin::menu(ui, id, locked, &|action| {
                dispatch_pin(action, &events, &ctx, None);
                let _ = events.send(Event::ClosePinMenu);
            });
        });
    }
}

impl eframe::App for Snip2Pin {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        i18n::set_locale(ctx, self.preferences.language.resolve(self.system_locale));
        if let Some(hint) = &self.unlock_hint {
            let remaining = hint.until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.clear_unlock_hint(ctx);
            } else {
                ctx.request_repaint_after_for(remaining, ViewportId::ROOT);
            }
        }
        if ctx.current_pass_index() == 0
            && ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| Binding::unlock_all().matches(event))
            })
        {
            self.unlock_pins(ctx);
        }
        if !matches!(self.phase, Phase::Idle)
            && ctx.input(|input| input.key_pressed(egui::Key::Escape))
        {
            self.cancel_capture(ctx);
        }
        self.update_icon(ctx);
        if let Err(error) = platform::prepare_floating_windows() {
            self.message = Some(error);
        }
        while let Ok(event) = self.events.try_recv() {
            self.handle_event(event, ctx);
        }
        let menu_state = self.menu_state();
        if let Some(tray) = &mut self.tray {
            tray.update(menu_state);
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            self.close_home(ctx);
        }
        self.prepare_quit_prompt(ctx, frame);
        #[cfg(target_os = "macos")]
        {
            let menu_state = self.menu_state();
            if let Some(dock) = &mut self.dock {
                dock.update(menu_state);
            }
        }
        self.show_pins(ctx);
        self.show_pin_menu(ctx);
        self.show_layers(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui::pin::route_gesture(ui, &|action| dispatch_pin(action, &self.sender, &ctx, None));
        if !self.prepare_home(&ctx) {
            return;
        }
        self.update_icon(&ctx);
        let idle = matches!(self.phase, Phase::Idle);
        let locale = self.preferences.language.resolve(self.system_locale);
        let message = self.message.as_ref().map(|message| message.render(locale));
        let pins = self
            .pins
            .iter()
            .rev()
            .filter(|(_, pin)| pin.first_frame_drawn.load(Ordering::Acquire))
            .map(|(&id, pin)| launcher::PinEntry {
                id,
                name: &pin.name,
                texture: &pin.texture,
                size: pin.pixels.size,
                display_size: ctx.input(|input| {
                    input
                        .raw
                        .viewports
                        .get(&pin_id(id))?
                        .inner_rect
                        .map(|rect| rect.size())
                }),
                locked: pin.locked,
                selected: pin_focused(&ctx, id),
            })
            .collect::<Vec<_>>();
        let action = ui
            .add_enabled_ui(self.quit_confirmation.is_none(), |ui| {
                launcher::show(
                    ui,
                    launcher::Model {
                        locale,
                        pins: &pins,
                        message: message.as_deref(),
                        global_shortcut: self
                            .shortcut
                            .as_ref()
                            .is_some_and(Shortcut::capture_registered),
                        preferences: self.preferences,
                        icon_texture: self.icon_texture.as_ref(),
                        recording: self.recording,
                        idle,
                        capturing: matches!(self.phase, Phase::Capturing),
                        demo: self.demo,
                        startup: self.startup.as_ref().map(|_| self.startup_enabled),
                        permission_required: self.permission == ScreenPermission::Required,
                    },
                )
            })
            .inner;
        match action {
            Some(launcher::Action::Capture) => self.capture_command(&ctx),
            Some(launcher::Action::ClosePins) => self.close_pins(&ctx),
            Some(launcher::Action::RecordShortcut) => {
                self.recording = !self.recording;
                self.message = None;
            }
            Some(launcher::Action::Appearance(theme, icon)) => {
                self.change_appearance(theme, icon, &ctx)
            }
            Some(launcher::Action::Language(language)) => self.change_language(language, &ctx),
            Some(launcher::Action::Startup(enabled)) => self.set_startup(enabled),
            Some(launcher::Action::OpenCaptureSettings) => {
                if let Err(error) = platform::open_capture_settings() {
                    self.message = Some(error);
                }
            }
            Some(launcher::Action::Restart) => self.restart(&ctx),
            Some(launcher::Action::SelectPin(id)) => self.select_pin(id, &ctx),
            Some(launcher::Action::RenamePin(id, name)) => {
                if let Some(pin) = self.pins.get_mut(&id) {
                    pin.name = name;
                }
            }
            Some(launcher::Action::Pin(action)) => dispatch_pin(action, &self.sender, &ctx, None),
            None => {}
        }
        if self.quit_confirmation.as_ref().is_some_and(|confirmation| {
            matches!(confirmation.presentation, QuitPresentation::Fallback)
        }) {
            let response = egui::Modal::new(egui::Id::new("quit-locked-pins")).show(&ctx, |ui| {
                ui.heading(locale.text(Text::QuitLockedTitle));
                ui.label(locale.text(Text::QuitLockedHelp));
                let mut confirm = false;
                ui.horizontal(|ui| {
                    if ui.button(locale.text(Text::Cancel)).clicked() {
                        self.cancel_quit(&ctx);
                    }
                    if ui.button(locale.text(Text::QuitConfirm)).clicked() {
                        confirm = true;
                    }
                });
                confirm
            });
            if response.inner {
                self.quit(&ctx);
            } else if response.should_close() {
                self.cancel_quit(&ctx);
            }
        }
        if self.recording && self.quit_confirmation.is_none() {
            self.record_key(ui);
        } else if idle
            && self.quit_confirmation.is_none()
            && !ctx.egui_wants_keyboard_input()
            && ui.input(|input| self.preferences.binding.capture_requested(&input.events))
        {
            self.capture_command(&ctx);
        }
    }
}

fn layer_id(display_id: u32) -> ViewportId {
    ViewportId::from_hash_of(("selection", display_id))
}

fn register_shortcut(
    binding: Binding,
    events: &Sender<Event>,
    ctx: &egui::Context,
) -> Result<(Shortcut, Option<Message>), Message> {
    let capture_events = events.clone();
    let unlock_events = events.clone();
    let capture_ctx = ctx.clone();
    let unlock_ctx = ctx.clone();
    Shortcut::register(
        binding,
        move || {
            let _ = capture_events.send(Event::Capture);
            capture_ctx.request_repaint_of(ViewportId::ROOT);
        },
        move || {
            let _ = unlock_events.send(Event::Tray(crate::tray::Action::UnlockPins));
            unlock_ctx.request_repaint_of(ViewportId::ROOT);
        },
    )
}

/// Read one work-area snapshot per operation; demo hosts never query the desktop.
fn display_areas(displays: &[Display], demo: bool) -> Vec<egui::Rect> {
    if demo {
        displays
            .iter()
            .map(|display| {
                egui::Rect::from_min_size(display.position().into(), display.logical_size().into())
            })
            .collect()
    } else {
        platform::pin_work_areas(displays)
    }
}

fn pin_focused(ctx: &egui::Context, id: u64) -> bool {
    ctx.input(|input| {
        input
            .raw
            .viewports
            .get(&pin_id(id))
            .and_then(|viewport| viewport.focused)
            == Some(true)
    })
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
