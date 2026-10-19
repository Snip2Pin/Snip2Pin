use super::*;

#[cfg(target_os = "macos")]
#[path = "tests/macos_drag.rs"]
mod macos_drag;

fn app() -> Snip2Pin {
    let (sender, events) = mpsc::channel();
    Snip2Pin {
        sender,
        events,
        phase: Phase::Idle,
        gate: CaptureGate::default(),
        capture_worker: None,
        layers: BTreeMap::new(),
        active_display: None,
        pins: BTreeMap::new(),
        pin_menu: None,
        quit_confirmation: None,
        next_quit_request: 0,
        #[cfg(target_os = "macos")]
        pin_drag: platform::macos_drag::DragEvents::default(),
        next_pin: 0,
        message: None,
        system_locale: Locale::Chinese,
        shortcut: None,
        preferences: Preferences::default(),
        applied_icon: None,
        icon_data: None,
        icon_texture: None,
        config_path: None,
        recording: false,
        suppress_shortcut_until: Instant::now(),
        demo: true,
        tray: None,
        #[cfg(target_os = "macos")]
        dock: None,
        startup: None,
        startup_enabled: false,
        exiting: false,
        home_visible: true,
        booting: false,
        permission: ScreenPermission::default(),
        relaunch: Arc::new(Mutex::new(None)),
    }
}

#[test]
fn language_changes_preserve_pins_preferences_and_retranslate_existing_messages() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    let directory = tempfile::tempdir().expect("isolated settings");
    app.config_path = Some(directory.path().join("preferences.toml"));
    app.preferences.theme = settings::Theme::Dark;
    app.preferences.icon = settings::Icon::Metal;
    let original = app.preferences;
    let pixels = app.pins[&1].pixels.clone();
    let initial = app.pins[&1].initial;
    app.message = Some(Text::ShortcutError.context(Text::ShortcutModifiers.into()));
    for (language, locale) in [
        (Language::English, Locale::English),
        (Language::Chinese, Locale::Chinese),
        (Language::System, Locale::Chinese),
    ] {
        app.change_language(language, &ctx);
        assert_eq!(i18n::locale(&ctx), locale);
        assert_eq!(
            app.preferences,
            Preferences {
                language,
                ..original
            }
        );
        assert_eq!(
            Preferences::load(app.config_path.as_ref().expect("path")).expect("saved"),
            app.preferences
        );
        assert!(Arc::ptr_eq(&pixels, &app.pins[&1].pixels));
        assert_eq!(app.pins[&1].initial, initial);
        let message = app
            .message
            .as_ref()
            .expect("existing message")
            .render(locale);
        assert!(message.contains(locale.text(Text::ShortcutModifiers)));
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
        output.textures_delta.clear();
        assert!(
            !output
                .viewport_output
                .values()
                .flat_map(|viewport| &viewport.commands)
                .any(|command| matches!(
                    command,
                    ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
                ))
        );
    }
}

#[test]
fn failed_language_save_keeps_the_previous_language() {
    let ctx = egui::Context::default();
    i18n::set_locale(&ctx, Locale::Chinese);
    let mut app = app();
    let directory = tempfile::tempdir().expect("isolated directory");
    app.config_path = Some(directory.path().to_owned());
    app.change_language(Language::English, &ctx);
    assert_eq!(app.preferences.language, Language::System);
    assert_eq!(i18n::locale(&ctx), Locale::Chinese);
    assert!(app.message.is_some());
}

#[test]
fn pending_capture_can_be_cancelled_before_the_backend_returns() {
    let ctx = egui::Context::default();
    let mut app = app();
    let id = app.gate.begin().expect("capture");
    app.phase = Phase::Capturing;
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| {
            eframe::App::logic(&mut app, ui.ctx(), &mut eframe::Frame::_new_kittest());
        },
    );
    output.textures_delta.clear();
    assert!(matches!(app.phase, Phase::Idle));
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    assert!(
        matches!(app.phase, Phase::Idle),
        "a late backend result cannot restart a cancelled selection"
    );
}

#[test]
fn capture_failure_is_visible_even_if_home_was_closed() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.home_visible = false;
    let id = app.gate.begin().expect("capture");
    app.phase = Phase::Capturing;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.accept_capture(id, Err("generated backend failure".into()), None, ctx);
    });
    assert_eq!(
        app.message
            .as_ref()
            .map(|message| message.render(Locale::English))
            .as_deref(),
        Some("generated backend failure")
    );
    assert!(app.home_visible);
    assert!(matches!(app.phase, Phase::Idle));
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Visible(true)));
}

#[test]
#[ignore = "generated 4K handoff benchmark; run explicitly in debug and release"]
fn capture_handoff_generated_4k_benchmark() {
    let mut timings = Vec::new();
    for _ in 0..3 {
        let ctx = egui::Context::default();
        ctx.input_mut(|input| input.max_texture_side = 8192);
        let mut app = app();
        let snapshots = (0..2)
            .map(|id| Snapshot {
                display: Display {
                    id,
                    origin: [id as f32 * 3840.0, 0.0],
                    size: [3840.0, 2160.0],
                    scale: 1.0,
                },
                pixels: Arc::new(ColorImage::filled(
                    [3840, 2160],
                    egui::Color32::from_rgb(50, 100, 150),
                )),
            })
            .collect();
        let id = app.gate.begin().expect("capture");
        app.phase = Phase::Capturing;
        let started = Instant::now();
        app.accept_capture(id, Ok(snapshots), None, &ctx);
        timings.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    eprintln!("generated 2x4K capture handoff, UI-thread milliseconds: {timings:?}");
}

#[test]
fn oversized_capture_shows_an_error_without_uploading_or_crashing() {
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.max_texture_side = 128);
    let mut app = app();
    let id = app.gate.begin().expect("capture");
    app.phase = Phase::Capturing;
    let mut snapshot = capture::demo();
    snapshot.pixels = Arc::new(ColorImage::filled([129, 1], egui::Color32::BLACK));
    app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.is_empty());
    assert!(
        app.message
            .as_ref()
            .is_some_and(|message| message.render(Locale::English).contains("128"))
    );
    assert!(app.home_visible);
}

#[test]
fn initial_display_fit_keeps_original_capture_pixels_for_copy() {
    let ctx = egui::Context::default();
    let mut app = app();
    let snapshot = capture::demo();
    let original = snapshot.pixels.clone();
    let display_size = snapshot.display.logical_size();
    let rect = geometry::PixelRect {
        x: 0,
        y: 0,
        width: original.size[0] as u32,
        height: original.size[1] as u32,
    };
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![snapshot]), None, &ctx);
    assert_eq!(
        app.layers[&0]
            .selection
            .as_ref()
            .expect("selection")
            .lock()
            .expect("selection lock")
            .pixel_step,
        [1, 1],
        "the generated desktop has a non-integer source density and keeps pixel steps"
    );
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.selection_action(session, 0, overlay::Action::Select(rect), ui.ctx());
    });
    output.textures_delta.clear();
    let pin = &app.pins[&1];
    assert!(pin.initial.width() <= display_size[0]);
    assert!(pin.screen.contains_rect(pin.initial));
    assert_eq!(pin.pixels.as_ref(), original.as_ref());
    assert!(
        matches!(&output.platform_output.commands[0], egui::OutputCommand::CopyImage(pixels) if pixels == original.as_ref())
    );
    let mut copy = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    copy.textures_delta.clear();
    assert!(
        matches!(&copy.platform_output.commands[0], egui::OutputCommand::CopyImage(pixels) if pixels == original.as_ref())
    );
}

#[test]
fn missing_permission_keeps_capture_idle_and_opens_an_actionable_home() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.demo = false;
    app.home_visible = false;
    let mut prompts = 0;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        assert!(!app.check_capture_permission(
            ctx,
            || false,
            || {
                prompts += 1;
                false
            }
        ));
        assert!(!app.check_capture_permission(ctx, || false, || panic!("no repeated prompt")));
    });
    assert_eq!(prompts, 1);
    assert!(matches!(app.phase, Phase::Idle));
    assert_eq!(app.permission, ScreenPermission::Required);
    assert!(app.home_visible);
    assert!(app.layers.is_empty());
    assert!(app.pins.is_empty());
    assert!(
        app.gate.begin().is_some(),
        "permission failure cannot occupy the capture gate"
    );
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Visible(true)));
    assert!(app.check_capture_permission(&ctx, || true, || panic!("already granted")));
    assert_eq!(app.permission, ScreenPermission::Granted);
}

#[test]
fn demo_never_requests_permission_or_relaunches_the_real_application() {
    let ctx = egui::Context::default();
    let mut app = app();
    assert!(app.check_capture_permission(
        &ctx,
        || panic!("demo must not preflight"),
        || panic!("demo must not request")
    ));
    app.restart(&ctx);
    assert!(!app.exiting);
    assert!(app.relaunch.lock().expect("relaunch handle").is_none());
}

#[test]
fn background_bootstrap_keeps_capture_layers_available_without_painting_home() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    app.home_visible = false;
    app.booting = true;
    let display = capture::demo().display;
    let display_id = display.id;
    app.layers.insert(
        display_id,
        Layer {
            display,
            selection: None,
            positioned: Arc::new(AtomicBool::new(false)),
        },
    );
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.show_layers(ui.ctx());
        assert!(!app.prepare_home(ui.ctx()));
    });
    output.textures_delta.clear();
    assert!(
        output.shapes.is_empty(),
        "background startup has no visible home contents"
    );
    assert!(!app.booting);
    assert!(
        matches!(app.phase, Phase::Idle),
        "startup never captures the desktop"
    );
    assert!(
        output.viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::Visible(false))
    );
    let layer = &output.viewport_output[&layer_id(display_id)];
    assert_eq!(layer.builder.mouse_passthrough, Some(true));
    assert_eq!(layer.builder.active, Some(false));
    assert_ne!(layer.builder.fullscreen, Some(true));
}

#[test]
fn tray_home_and_quit_dispatch_root_commands_and_demo_cannot_enable_startup() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.home_visible = false;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Home, ctx)
    });
    assert!(app.home_visible);
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Visible(true)));
    app.set_startup(true);
    assert!(!app.startup_enabled, "demo never registers startup");
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx)
    });
    assert!(app.exiting);
    assert!(output.viewport_commands[&ViewportId::ROOT].contains(&ViewportCommand::Close));
}

#[test]
fn opening_home_without_a_confirmation_restores_normal_window_level() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Home, ctx)
    });
    let commands = &output.viewport_commands[&ViewportId::ROOT];
    assert!(commands.contains(&ViewportCommand::WindowLevel(egui::WindowLevel::Normal)));
    assert!(!commands.contains(&ViewportCommand::WindowLevel(
        egui::WindowLevel::AlwaysOnTop
    )));
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
fn selection_layer_does_not_retry_native_position_changes() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    let viewport = layer_id(0);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
    setup.textures_delta.clear();
    let callback = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("selection callback");
    for index in 0..3 {
        let mut input = egui::RawInput {
            viewport_id: viewport,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(680.0, 420.0),
            )),
            ..Default::default()
        };
        input.viewports.insert(
            viewport,
            egui::ViewportInfo {
                outer_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(84.0, 104.0),
                    egui::vec2(680.0, 420.0),
                )),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| callback(ui));
        output.textures_delta.clear();
        let count = output.viewport_output[&viewport]
            .commands
            .iter()
            .filter(|command| matches!(command, ViewportCommand::OuterPosition(_)))
            .count();
        assert_eq!(count, usize::from(!cfg!(target_os = "macos") && index == 0));
    }
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
    assert_eq!(
        pin.initial.min,
        egui::pos2(origin[0] + 68.0, origin[1] + 42.0)
    );
    assert_eq!(pin.initial.size(), egui::vec2(136.0, 84.0));
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

#[test]
fn successful_selection_copies_only_cropped_pixels_once_and_can_copy_again() {
    let ctx = egui::Context::default();
    let mut app = app();
    let snapshot = capture::demo();
    let rect = geometry::PixelRect {
        x: 3,
        y: 5,
        width: 7,
        height: 9,
    };
    let expected = capture::crop(&snapshot.pixels, rect).expect("valid crop");
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.selection_action(id + 1, 0, overlay::Action::Select(rect), ui.ctx());
        assert!(
            app.pins.is_empty(),
            "stale selections cannot copy or create pins"
        );
        app.selection_action(id, 0, overlay::Action::Select(rect), ui.ctx());
        app.selection_action(id, 0, overlay::Action::Select(rect), ui.ctx());
    });
    output.textures_delta.clear();
    assert_eq!(output.platform_output.commands.len(), 1);
    assert!(
        matches!(&output.platform_output.commands[0], egui::OutputCommand::CopyImage(image) if image == &expected)
    );
    assert_eq!(app.pins[&1].pixels.as_ref(), &expected);

    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    output.textures_delta.clear();
    assert_eq!(output.platform_output.commands.len(), 1);
    assert!(
        matches!(&output.platform_output.commands[0], egui::OutputCommand::CopyImage(image) if image == &expected)
    );
    app.pins.remove(&1);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    output.textures_delta.clear();
    assert!(
        output.platform_output.commands.is_empty(),
        "closed pins cannot overwrite the clipboard"
    );
}

#[test]
fn canceled_and_invalid_selections_leave_the_clipboard_untouched() {
    for rect in [
        None,
        Some(geometry::PixelRect {
            x: 0,
            y: 0,
            width: 0,
            height: 1,
        }),
        Some(geometry::PixelRect {
            x: u32::MAX,
            y: 1,
            width: 1,
            height: 1,
        }),
        Some(geometry::PixelRect {
            x: 0,
            y: 0,
            width: 0,
            height: 8,
        }),
        Some(geometry::PixelRect {
            x: 0,
            y: 0,
            width: 8,
            height: 0,
        }),
    ] {
        let ctx = egui::Context::default();
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.accept_capture(id, Ok(vec![capture::demo()]), None, ui.ctx());
            app.selection_action(
                id,
                0,
                rect.map_or(overlay::Action::Cancel, overlay::Action::Select),
                ui.ctx(),
            );
        });
        output.textures_delta.clear();
        assert!(output.platform_output.commands.is_empty());
        assert!(app.pins.is_empty());
    }
}

#[test]
fn single_pixel_selections_copy_the_original_without_an_application_size_floor() {
    for scale in [1.0_f32, 1.5, 2.0, 3.0] {
        for valid in [false, true] {
            let ctx = egui::Context::default();
            let mut app = app();
            let session = app.gate.begin().expect("capture");
            let mut snapshot = capture::demo();
            snapshot.display.scale = scale;
            snapshot.display.size = if cfg!(target_os = "macos") {
                [64.0; 2]
            } else {
                [64.0 * scale; 2]
            };
            snapshot.pixels = Arc::new(ColorImage::filled(
                [(64.0 * scale) as usize; 2],
                egui::Color32::WHITE,
            ));
            let edge = u32::from(valid);
            let rect = geometry::PixelRect {
                x: 0,
                y: 0,
                width: edge,
                height: edge,
            };
            app.accept_capture(session, Ok(vec![snapshot]), None, &ctx);
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.selection_action(session, 0, overlay::Action::Select(rect), ui.ctx());
            });
            output.textures_delta.clear();
            assert_eq!(app.pins.len(), usize::from(valid));
            assert_eq!(output.platform_output.commands.len(), usize::from(valid));
            if valid {
                assert_eq!(app.pins[&1].pixels.size, [edge as usize; 2]);
                assert!((app.pins[&1].initial.width() - scale.recip()).abs() < 0.00001);
                assert!((app.pins[&1].initial.height() - scale.recip()).abs() < 0.00001);
            }
        }
    }
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
    snapshot.pixels = Arc::new(ColorImage::filled([2880, 1800], egui::Color32::TRANSPARENT));
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
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
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
fn native_image_paints_observed_geometry_without_repositioning_or_resize_compensation() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let texture = app.pins[&1].texture.id();
    let original = app.pins[&1].pixels.clone();
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("image callback");
    for (index, actual) in [
        egui::Rect::from_min_size(egui::pos2(104.0, 103.0), egui::vec2(193.0, 121.0)),
        egui::Rect::from_min_size(egui::pos2(54.0, 103.0), egui::vec2(243.0, 151.0)),
        egui::Rect::from_min_size(egui::pos2(54.0, 63.0), egui::vec2(283.0, 191.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = egui::RawInput {
            viewport_id: pin_id(1),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
            ..Default::default()
        };
        input.viewports.insert(
            pin_id(1),
            egui::ViewportInfo {
                inner_rect: Some(actual),
                outer_rect: Some(actual),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| {
            paint(ui);
            if index == 1 && ui.ctx().current_pass_index() == 0 {
                ui.ctx().request_discard("repeat native image pass");
            }
        });
        output.textures_delta.clear();
        let painted = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
                _ => None,
            })
            .expect("image painted");
        assert_eq!(
            painted,
            egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())
        );
        assert!(
            output.viewport_output[&pin_id(1)]
                .commands
                .iter()
                .all(|command| !matches!(
                    command,
                    ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
                ))
        );
    }
    assert_eq!(
        app.events
            .try_iter()
            .filter(|event| matches!(event, Event::PinReady(_, 1)))
            .count(),
        1
    );
    let mut copied = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    copied.textures_delta.clear();
    assert!(
        matches!(&copied.platform_output.commands[0], egui::OutputCommand::CopyImage(pixels) if pixels == original.as_ref())
    );
}

#[test]
fn top_edge_pin_finishes_capture_when_the_system_moves_it_below_the_menu_bar() {
    for height in [120, 600] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let id = app.gate.begin().expect("capture starts");
        let mut snapshot = capture::demo();
        snapshot.display.origin = [0.0, 0.0];
        app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
        app.selection_action(
            id,
            0,
            overlay::Action::Select(geometry::PixelRect {
                x: 96,
                y: 0,
                width: 192,
                height,
            }),
            &ctx,
        );
        let viewport = ViewportId::from_hash_of(("pin", 1_u64));
        let texture = app.pins[&1].texture.id();
        let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
        setup.textures_delta.clear();
        let output = &setup.viewport_output[&viewport];
        let paint = output.viewport_ui_cb.as_ref().expect("pin callback");
        let mut actual = egui::Rect::from_min_size(
            output.builder.position.expect("position"),
            output.builder.inner_size.expect("size"),
        );
        let mut painted = false;
        let mut ready = false;
        for _ in 0..4 {
            actual = actual.translate(egui::vec2(0.0, (28.0 - actual.min.y).max(0.0)));
            let mut input = egui::RawInput {
                viewport_id: viewport,
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
                ..Default::default()
            };
            input.viewports.insert(
                viewport,
                egui::ViewportInfo {
                    inner_rect: Some(actual),
                    outer_rect: Some(actual),
                    ..Default::default()
                },
            );
            let mut frame = ctx.run_ui(input, |ui| paint(ui));
            frame.textures_delta.clear();
            painted |= frame.shapes.iter().any(|shape| {
                matches!(&shape.shape,
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture)
            });
            ready |= app
                .events
                .try_iter()
                .any(|event| matches!(event, Event::PinReady(session, 1) if session == id));
            for command in &frame.viewport_output[&viewport].commands {
                match command {
                    ViewportCommand::InnerSize(size) => {
                        actual = egui::Rect::from_min_size(actual.min, *size)
                    }
                    ViewportCommand::OuterPosition(position) => {
                        actual = egui::Rect::from_min_size(*position, actual.size())
                    }
                    _ => {}
                }
            }
        }
        assert!(
            ready && painted,
            "top-edge capture must reveal the moved pin and finish, instead of endlessly retrying an impossible desktop position"
        );
        app.restore(&ctx);
        assert!(matches!(app.phase, Phase::Idle));
        assert!(app.layers.values().all(|layer| layer.selection.is_none()));
    }
}

#[test]
fn controls_share_the_image_viewport_and_follow_hover_and_visible_image_edges() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let area = egui::Rect::from_min_max(egui::pos2(0.0, 30.0), egui::pos2(680.0, 420.0));
    app.pins.get_mut(&1).expect("pin").screen = area;
    app.pins.get_mut(&1).expect("pin").screen_areas = Arc::new(vec![area]);
    // Supply a menu bar and Dock work area before capturing the callback.
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("image callback");
    for position in [
        egui::pos2(100.0, 100.0),
        egui::pos2(100.0, -30.0),
        egui::pos2(580.0, 100.0),
        egui::pos2(-100.0, 100.0),
        egui::pos2(100.0, 350.0),
        egui::pos2(580.0, -30.0),
    ] {
        let actual = egui::Rect::from_min_size(position, egui::vec2(193.0, 121.0));
        let row = pin::toolbar_geometry(actual, &[area], area, 1.0).expect("reachable buttons");
        let local_row = row.translate(-actual.min.to_vec2());
        assert!(area.contains_rect(row.bounds));
        assert!(actual.contains_rect(row.bounds));
        for (events, hovered) in [
            (vec![egui::Event::PointerGone], false),
            (
                vec![egui::Event::PointerMoved(local_row.bounds.center())],
                true,
            ),
            (vec![egui::Event::PointerGone], false),
        ] {
            let mut input = egui::RawInput {
                viewport_id: pin_id(1),
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
                events,
                ..Default::default()
            };
            input.viewports.insert(
                pin_id(1),
                egui::ViewportInfo {
                    inner_rect: Some(actual),
                    outer_rect: Some(actual),
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |ui| paint(ui));
            output.textures_delta.clear();
            assert!(
                output
                    .viewport_output
                    .keys()
                    .all(|id| *id == pin_id(1) || *id == ViewportId::ROOT)
            );
            let buttons: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.size() == egui::Vec2::splat(pin::METRICS.button) =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(buttons.len(), if hovered { row.count } else { 0 });
            for button in buttons {
                assert!(local_row.bounds.contains_rect(button));
            }
            assert!(
                output.viewport_output[&pin_id(1)]
                    .commands
                    .iter()
                    .all(|command| !matches!(
                        command,
                        ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
                    ))
            );
        }
    }
}

#[test]
fn controls_stay_inside_resized_and_zoomed_viewports_without_global_geometry() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 480,
            height: 300,
        }),
        &ctx,
    );
    let texture = app.pins[&1].texture.id();
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    for zoom in [1.0, 1.25, 2.0] {
        ctx.set_zoom_factor(zoom);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
        warmup.textures_delta.clear();
        for size in [
            egui::vec2(480.0, 300.0),
            egui::vec2(80.0, 50.0),
            egui::vec2(120.0, 70.0),
            egui::vec2(640.0, 400.0),
        ] {
            let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            for hovered in [true, false] {
                let mut input = egui::RawInput {
                    viewport_id: pin_id(1),
                    screen_rect: Some(bounds),
                    events: vec![if hovered {
                        egui::Event::PointerMoved(bounds.center())
                    } else {
                        egui::Event::PointerGone
                    }],
                    ..Default::default()
                };
                input.viewports.insert(
                    pin_id(1),
                    egui::ViewportInfo {
                        native_pixels_per_point: Some(1.5),
                        inner_rect: None,
                        outer_rect: None,
                        ..Default::default()
                    },
                );
                let mut output = ctx.run_ui(input, |ui| {
                    paint(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("repeat local viewport pass");
                    }
                });
                output.textures_delta.clear();
                assert_eq!(output.pixels_per_point, 1.5 * zoom);
                let image = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                        Some(mesh.calc_bounds())
                    }
                    _ => None,
                });
                assert_eq!(image, Some(bounds));
                let buttons: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.size() == egui::Vec2::splat(pin::METRICS.button) =>
                        {
                            Some((rect.rect, shape.clip_rect))
                        }
                        _ => None,
                    })
                    .collect();
                let count = pin::toolbar_geometry(bounds, &[], bounds, 1.0)
                    .map_or(0, |toolbar| toolbar.count);
                assert_eq!(buttons.len(), if hovered { count } else { 0 });
                for (button, clip) in buttons {
                    assert!(
                        bounds.contains_rect(button) && clip.contains_rect(button),
                        "controls must remain fully visible without global bounds: zoom={zoom}, size={size:?}, button={button:?}"
                    );
                }
                assert!(output.viewport_output.values().all(|viewport| {
                    viewport.commands.iter().all(|command| {
                        !matches!(
                            command,
                            ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
                        )
                    })
                }));
            }
        }
    }
}

#[test]
fn pending_pin_can_be_closed_before_native_geometry_is_ready() {
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
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let viewport = ViewportId::from_hash_of(("pin", 1_u64));
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    let mut input = egui::RawInput {
        viewport_id: viewport,
        ..Default::default()
    };
    input.viewports.insert(
        viewport,
        egui::ViewportInfo {
            events: vec![egui::ViewportEvent::Close],
            ..Default::default()
        },
    );
    let mut frame = ctx.run_ui(input, |ui| paint(ui));
    frame.textures_delta.clear();
    assert!(matches!(app.events.try_recv(), Ok(Event::ClosePin(1))));
    app.close_pin(1, &ctx);
    assert!(app.pins.is_empty());
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
}

#[test]
fn stale_unlocked_callback_cannot_close_a_newly_locked_pin() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.applied_icon = Some(app.preferences.icon_variant(ctx.system_theme()));
    let viewport = pin_id(1);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&viewport]
        .viewport_ui_cb
        .as_ref()
        .expect("unlocked callback");
    app.toggle_pin_lock(1, &ctx);
    let mut input = egui::RawInput {
        viewport_id: viewport,
        ..Default::default()
    };
    input.viewports.insert(
        viewport,
        egui::ViewportInfo {
            events: vec![egui::ViewportEvent::Close],
            ..Default::default()
        },
    );
    let mut child = ctx.run_ui(input, |ui| paint(ui));
    child.textures_delta.clear();
    // eframe retains a child window while root logic still registers its viewport.
    let mut root = ctx.run_ui(egui::RawInput::default(), |ui| {
        eframe::App::logic(&mut app, ui.ctx(), &mut eframe::Frame::_new_kittest());
    });
    root.textures_delta.clear();
    assert!(app.pins[&1].locked);
    assert!(root.viewport_output.contains_key(&viewport));
    assert!(
        !root.viewport_output[&viewport]
            .commands
            .contains(&ViewportCommand::Close)
    );
}

#[test]
fn cancelling_capture_preserves_a_locked_pending_pin() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let session = app.gate.begin().expect("capture starts");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let pixels = Arc::downgrade(&app.pins[&1].pixels);
    app.toggle_pin_lock(1, &ctx);
    app.cancel_capture(&ctx);
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
    assert!(app.pins[&1].locked);
    assert!(pixels.upgrade().is_some());
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    output.textures_delta.clear();
    assert!(output.viewport_output.contains_key(&pin_id(1)));
    app.toggle_pin_lock(1, &ctx);
    app.close_pin(1, &ctx);
    assert!(
        pixels.upgrade().is_none(),
        "unlock and close releases the original"
    );
}

#[test]
fn selection_escape_cancels_a_pending_pin_and_releases_capture_layers() {
    let ctx = egui::Context::default();
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    app.accept_capture(id, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        id,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    assert_eq!(app.pins.len(), 1);
    app.selection_action(id, 0, overlay::Action::Cancel, &ctx);
    assert!(app.pins.is_empty());
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
}

#[test]
fn root_escape_cancels_pending_pin_before_a_late_ready_event_and_allows_recapture() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    app.applied_icon = Some(app.preferences.icon_variant(ctx.system_theme()));
    let session = app.gate.begin().expect("capture starts");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 0,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let pixels = Arc::downgrade(&app.pins[&1].pixels);
    let mut cancelled = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| eframe::App::logic(&mut app, ui.ctx(), &mut eframe::Frame::_new_kittest()),
    );
    cancelled.textures_delta.clear();
    assert!(app.pins.is_empty());
    assert!(
        pixels.upgrade().is_none(),
        "cancellation releases the cropped pixels"
    );
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));

    let next = app.gate.begin().expect("capture can start immediately");
    app.accept_capture(next, Ok(vec![capture::demo()]), None, &ctx);
    app.sender
        .send(Event::PinReady(session, 1))
        .expect("late event");
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        eframe::App::logic(&mut app, ui.ctx(), &mut eframe::Frame::_new_kittest());
    });
    output.textures_delta.clear();
    assert!(matches!(app.phase, Phase::Selecting { id, pending_pin: None, .. } if id == next));
    assert!(app.pins.is_empty());
    assert!(!output.viewport_output.contains_key(&pin_id(1)));
    assert!(
        output
            .platform_output
            .commands
            .iter()
            .all(|command| !matches!(command, egui::OutputCommand::CopyImage(_)))
    );
}

#[test]
fn native_image_size_and_origin_are_taken_from_each_viewports_observed_bounds() {
    for scale in [1.0, 1.5, 2.0] {
        for zoom in [1.0, 1.25] {
            let ctx = egui::Context::default();
            ctx.set_embed_viewports(false);
            ctx.set_zoom_factor(zoom);
            let mut app = app();
            let session = app.gate.begin().expect("capture");
            let mut snapshot = capture::demo();
            snapshot.display.scale = scale;
            app.accept_capture(session, Ok(vec![snapshot]), None, &ctx);
            app.selection_action(
                session,
                0,
                overlay::Action::Select(geometry::PixelRect {
                    x: 97,
                    y: 61,
                    width: 193,
                    height: 121,
                }),
                &ctx,
            );
            let texture = app.pins[&1].texture.id();
            let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
            setup.textures_delta.clear();
            let paint = setup.viewport_output[&pin_id(1)]
                .viewport_ui_cb
                .as_ref()
                .expect("callback");
            let actual =
                egui::Rect::from_min_size(egui::pos2(-500.0, 31.0), egui::vec2(243.0, 157.0));
            let mut input = egui::RawInput {
                viewport_id: pin_id(1),
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
                ..Default::default()
            };
            input.viewports.insert(
                pin_id(1),
                egui::ViewportInfo {
                    native_pixels_per_point: Some(scale),
                    inner_rect: Some(actual),
                    outer_rect: Some(actual),
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |ui| paint(ui));
            output.textures_delta.clear();
            let image = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                        Some(mesh.calc_bounds())
                    }
                    _ => None,
                })
                .expect("image");
            assert_eq!(image.min, egui::Pos2::ZERO);
            assert_eq!(image.size(), actual.size());
            assert!(
                output.viewport_output[&pin_id(1)]
                    .commands
                    .iter()
                    .all(|command| !matches!(
                        command,
                        ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
                    ))
            );
        }
    }
}

#[test]
fn native_dpi_and_keyboard_zoom_never_add_a_button_driven_image_minimum() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 192,
            height: 120,
        }),
        &ctx,
    );
    let original = app.pins[&1].pixels.clone();
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    let actual = egui::Rect::from_min_size(egui::pos2(-300.0, 31.0), egui::vec2(243.0, 157.0));
    let run = |scale, events, repeat| {
        let mut input = egui::RawInput {
            viewport_id: pin_id(1),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
            events,
            ..Default::default()
        };
        input.viewports.insert(
            pin_id(1),
            egui::ViewportInfo {
                native_pixels_per_point: Some(scale),
                inner_rect: Some(actual),
                outer_rect: Some(actual),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| {
            paint(ui);
            if repeat && ui.ctx().current_pass_index() == 0 {
                ui.ctx()
                    .request_discard("repeat image pass after DPI or zoom changes");
            }
        });
        output.textures_delta.clear();
        assert!(
            output
                .viewport_output
                .values()
                .all(|viewport| viewport.commands.iter().all(|command| !matches!(
                    command,
                    ViewportCommand::InnerSize(_)
                        | ViewportCommand::OuterPosition(_)
                        | ViewportCommand::MinInnerSize(_)
                )))
        );
        output
    };
    for scale in [1.0, 1.5, 2.0, 1.0] {
        let changed = run(scale, vec![], true);
        assert_eq!(changed.pixels_per_point, scale);
        assert_eq!(changed.platform_output.num_completed_passes, 2);
        let unchanged = run(scale, vec![], true);
        assert_eq!(unchanged.platform_output.num_completed_passes, 2);
    }

    assert!(ctx.options(|options| options.zoom_with_keyboard));
    run(
        1.0,
        vec![egui::Event::Key {
            key: egui::Key::Plus,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        false,
    );
    let zoomed = run(1.0, vec![], true);
    assert!((ctx.zoom_factor() - 1.1).abs() < 0.001);
    assert!((zoomed.pixels_per_point - 1.1).abs() < 0.001);
    run(1.0, vec![], true);
    assert_eq!(
        app.events
            .try_iter()
            .filter(|event| matches!(event, Event::PinReady(id, 1) if *id == session))
            .count(),
        1
    );
    let mut copied = ctx.run_ui(egui::RawInput::default(), |ui| app.copy_pin(1, ui.ctx()));
    copied.textures_delta.clear();
    assert_eq!(copied.platform_output.commands.len(), 1);
    assert!(
        matches!(&copied.platform_output.commands[0], egui::OutputCommand::CopyImage(pixels) if pixels == original.as_ref())
    );
}

#[test]
fn pin_window_is_created_at_the_selection_origin_without_changing_root() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let id = app.gate.begin().expect("capture starts");
    let snapshot = capture::demo();
    let rect = geometry::PixelRect {
        x: 96,
        y: 60,
        width: 192,
        height: 120,
    };
    let placement = geometry::pin_placement(
        rect,
        snapshot.pixel_size(),
        snapshot.display.position(),
        snapshot.display.logical_size(),
    );
    let screen = egui::Rect::from_min_size(
        snapshot.display.position().into(),
        snapshot.display.logical_size().into(),
    );
    let layout = pin::frame_geometry(placement, screen, 1.0);
    app.accept_capture(id, Ok(vec![snapshot]), None, &ctx);
    app.selection_action(id, 0, overlay::Action::Select(rect), &ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.show_pins(ui.ctx());
    });
    output.textures_delta.clear();
    let pin = output
        .viewport_output
        .get(&ViewportId::from_hash_of(("pin", 1_u64)))
        .expect("native pin");
    assert_eq!(pin.builder.position, Some(layout.image.min));
    assert_eq!(pin.builder.inner_size, Some(layout.image.size()));
    assert_eq!(pin.builder.resizable, Some(true));
    assert!(pin.builder.min_inner_size.is_none());
    assert_eq!(pin.builder.has_shadow, Some(true));
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
fn narrow_capture_keeps_its_display_size_and_text_density() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = app();
    let session = app.gate.begin().expect("capture starts");
    let snapshot = capture::demo();
    let rect = geometry::PixelRect {
        x: 180,
        y: 120,
        width: 24,
        height: 240,
    };
    let expected = geometry::pin_placement(
        rect,
        snapshot.pixel_size(),
        snapshot.display.position(),
        snapshot.display.logical_size(),
    );
    app.accept_capture(session, Ok(vec![snapshot]), None, &ctx);
    app.selection_action(session, 0, overlay::Action::Select(rect), &ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    output.textures_delta.clear();
    let builder = &output.viewport_output[&pin_id(1)].builder;
    let shown = builder.inner_size.expect("native image size");
    assert!(
        (shown - egui::Vec2::from(expected.size)).length() < 0.001,
        "controls must adapt to the image, not enlarge it: selected={:?}, shown={shown:?}",
        expected.size,
    );
    assert!(builder.min_inner_size.is_none());
    assert_eq!(app.pins[&1].pixels.size, [24, 240]);
}

#[cfg(target_os = "macos")]
#[test]
fn retina_overlay_canonicalizes_odd_pixels_before_copy_and_initial_placement() {
    for zoom in [1.0, 1.25, 2.0] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        ctx.set_zoom_factor(zoom);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
        warmup.textures_delta.clear();
        let mut app = app();
        let session = app.gate.begin().expect("capture starts");
        let snapshot = Snapshot {
            display: Display {
                id: 0,
                origin: [0.0, 0.0],
                size: [1000.0, 800.0],
                scale: 2.0,
            },
            pixels: Arc::new(ColorImage::filled([2000, 1600], egui::Color32::WHITE)),
        };
        let source = snapshot.pixels.clone();
        app.accept_capture(session, Ok(vec![snapshot]), None, &ctx);
        assert_eq!(
            app.layers[&0]
                .selection
                .as_ref()
                .expect("selection")
                .lock()
                .expect("lock")
                .pixel_step,
            [2, 2]
        );
        let mut layers = ctx.run_ui(egui::RawInput::default(), |ui| app.show_layers(ui.ctx()));
        layers.textures_delta.clear();
        let paint = layers.viewport_output[&layer_id(0)]
            .viewport_ui_cb
            .as_ref()
            .expect("selection callback");
        let start = egui::pos2(100.5, 150.5) / zoom;
        let end = egui::pos2(400.5, 350.5) / zoom;
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut selected = None;
        let mut copies = Vec::new();
        for events in [
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, true)],
            vec![egui::Event::PointerMoved(end)],
            vec![button(end, false)],
        ] {
            let bounds =
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0) / zoom);
            let mut input = egui::RawInput {
                viewport_id: layer_id(0),
                screen_rect: Some(bounds),
                events,
                ..Default::default()
            };
            input.viewports.insert(
                layer_id(0),
                egui::ViewportInfo {
                    native_pixels_per_point: Some(2.0),
                    inner_rect: Some(bounds),
                    outer_rect: Some(bounds),
                    ..Default::default()
                },
            );
            let mut frame = ctx.run_ui(input, |ui| {
                paint(ui);
                for event in app.events.try_iter().collect::<Vec<_>>() {
                    if let Event::Selection(id, display, action) = event {
                        if let overlay::Action::Select(rect) = &action {
                            selected = Some(*rect);
                        }
                        app.selection_action(id, display, action, ui.ctx());
                    }
                }
            });
            frame.textures_delta.clear();
            copies.extend(
                frame
                    .platform_output
                    .commands
                    .into_iter()
                    .filter_map(|command| match command {
                        egui::OutputCommand::CopyImage(image) => Some(image),
                        _ => None,
                    }),
            );
        }
        let rect = geometry::PixelRect {
            x: 200,
            y: 300,
            width: 602,
            height: 402,
        };
        assert_eq!(selected, Some(rect));
        assert_eq!(app.pins[&1].pixels.size, [602, 402]);
        assert_eq!(
            copies,
            [capture::crop(&source, rect).expect("canonical source crop")]
        );
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
        output.textures_delta.clear();
        let builder = &output.viewport_output[&pin_id(1)].builder;
        assert!(
            (builder.position.expect("initial origin") * zoom - egui::pos2(100.0, 150.0)).length()
                < 0.001,
            "overlay crop and native placement must share one canonical origin at zoom={zoom}"
        );
        assert!(
            (builder.inner_size.expect("initial size") * zoom - egui::vec2(301.0, 201.0)).length()
                < 0.001
        );
    }
}

#[test]
fn pin_builder_cancels_backend_zoom_without_enlarging_tiny_images_for_controls() {
    for scale in [1.0, 1.5, 2.0] {
        for zoom in [1.0, 1.25, 2.0] {
            for tiny in [false, true] {
                let ctx = egui::Context::default();
                ctx.set_embed_viewports(false);
                ctx.set_zoom_factor(zoom);
                let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
                warmup.textures_delta.clear();
                let mut app = app();
                let session = app.gate.begin().expect("capture");
                let mut snapshot = capture::demo();
                snapshot.display.scale = scale;
                let rect = geometry::PixelRect {
                    x: 96,
                    y: 60,
                    width: if tiny { 8 } else { 480 },
                    height: if tiny { 5 } else { 300 },
                };
                let placement = geometry::pin_placement(
                    rect,
                    snapshot.pixel_size(),
                    snapshot.display.position(),
                    snapshot.display.logical_size(),
                );
                app.accept_capture(session, Ok(vec![snapshot]), None, &ctx);
                app.selection_action(session, 0, overlay::Action::Select(rect), &ctx);
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    app.show_pins(ui.ctx());
                });
                output.textures_delta.clear();
                let builder = &output.viewport_output[&pin_id(1)].builder;
                let position = builder.position.expect("initial position");
                let size = builder.inner_size.expect("initial size");
                let model = &app.pins[&1];
                assert!(builder.min_inner_size.is_none());
                assert_eq!(
                    model.pixels.size,
                    [rect.width as usize, rect.height as usize]
                );
                let unzoomed = pin::frame_geometry(
                    placement,
                    model.screen,
                    pin::coordinate_ratio(scale, 1.0, scale),
                )
                .image;
                assert!((position * zoom).distance(unzoomed.min) < 0.001);
                assert!((size * zoom - unzoomed.size()).length() < 0.001);
                assert!((size * zoom - egui::Vec2::from(placement.size)).length() < 0.001);
                if tiny {
                    let image = egui::Rect::from_min_size(position, size);
                    assert!(
                        pin::toolbar_geometry(
                            image,
                            &model.screen_areas,
                            model.screen,
                            zoom.recip()
                        )
                        .is_none(),
                        "tiny images use the independent menu rather than being enlarged at scale={scale}, zoom={zoom}"
                    );
                }
            }
        }
    }
}

#[test]
fn changing_zoom_keeps_existing_pin_builder_and_paints_only_observed_native_geometry() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx.set_zoom_factor(1.25);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
    warmup.textures_delta.clear();
    let mut app = app();
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, &ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 96,
            y: 60,
            width: 480,
            height: 300,
        }),
        &ctx,
    );
    let texture = app.pins[&1].texture.id();
    let mut initial = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    initial.textures_delta.clear();
    let initial_builder = initial.viewport_output[&pin_id(1)].builder.clone();
    ctx.set_zoom_factor(2.0);
    let mut changed = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    changed.textures_delta.clear();
    let builder = &changed.viewport_output[&pin_id(1)].builder;
    assert_eq!(builder.position, initial_builder.position);
    assert_eq!(builder.inner_size, initial_builder.inner_size);
    assert!(builder.min_inner_size.is_none());
    let paint = changed.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback");
    let actual = egui::Rect::from_min_size(egui::pos2(-300.0, 31.0), egui::vec2(243.0, 157.0));
    let mut input = egui::RawInput {
        viewport_id: pin_id(1),
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size())),
        ..Default::default()
    };
    input.viewports.insert(
        pin_id(1),
        egui::ViewportInfo {
            inner_rect: Some(actual),
            outer_rect: Some(actual),
            ..Default::default()
        },
    );
    let mut output = ctx.run_ui(input, |ui| {
        paint(ui);
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx()
                .request_discard("repeat changed-zoom native image pass");
        }
    });
    output.textures_delta.clear();
    let painted = output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
        _ => None,
    });
    assert_eq!(
        painted,
        Some(egui::Rect::from_min_size(egui::Pos2::ZERO, actual.size()))
    );
    assert!(
        output
            .viewport_output
            .values()
            .all(|viewport| viewport.commands.iter().all(|command| !matches!(
                command,
                ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
            )))
    );
}

fn tiny_pin_app(ctx: &egui::Context) -> Snip2Pin {
    let mut app = app();
    let session = app.gate.begin().expect("capture");
    app.accept_capture(session, Ok(vec![capture::demo()]), None, ctx);
    app.selection_action(
        session,
        0,
        overlay::Action::Select(geometry::PixelRect {
            x: 12,
            y: 18,
            width: 3,
            height: 5,
        }),
        ctx,
    );
    app.restore(ctx);
    let mut selected = ctx.run_ui(egui::RawInput::default(), |_| {});
    selected.textures_delta.clear();
    app
}

fn add_second_test_pin(app: &mut Snip2Pin) {
    let pin = &app.pins[&1];
    let second = Pin {
        name: String::new(),
        texture: pin.texture.clone(),
        pixels: pin.pixels.clone(),
        initial: pin.initial,
        display_scale: pin.display_scale,
        screen: pin.screen,
        screen_areas: pin.screen_areas.clone(),
        session: pin.session,
        ready: Arc::new(AtomicBool::new(true)),
        locked: false,
        #[cfg(target_os = "macos")]
        presented: Cell::new(false),
    };
    app.pins.insert(2, second);
    app.pins[&1].ready.store(true, Ordering::Release);
}

#[test]
fn lock_state_guards_queued_close_events_and_close_all_until_explicit_unlock() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    add_second_test_pin(&mut app);
    let original = app.pins[&1].pixels.clone();
    let initial = app.pins[&1].initial;
    app.sender
        .send(Event::ClosePin(1))
        .expect("queued before lock");
    let locked = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.toggle_pin_lock(1, ctx)
    });
    assert!(app.pins[&1].locked);
    assert!(locked.viewport_commands[&pin_id(1)].contains(&ViewportCommand::Resizable(false)));
    let guarded = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        app.tray_action(crate::tray::Action::ClosePin(1), ctx);
        app.close_pin(1, ctx);
        app.tray_action(crate::tray::Action::ClosePins, ctx);
    });
    assert_eq!(app.pins.keys().copied().collect::<Vec<_>>(), [1]);
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    assert_eq!(app.pins[&1].initial, initial);
    assert!(
        !guarded
            .viewport_commands
            .get(&pin_id(1))
            .is_some_and(|commands| commands.contains(&ViewportCommand::Close))
    );
    assert!(guarded.viewport_commands[&pin_id(2)].contains(&ViewportCommand::Close));
    let unlocked = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.toggle_pin_lock(1, ctx)
    });
    assert!(!app.pins[&1].locked);
    assert!(unlocked.viewport_commands[&pin_id(1)].contains(&ViewportCommand::Resizable(true)));
    let closed = ctx.run_logic(&egui::RawInput::default(), |ctx| app.close_pins(ctx));
    assert!(app.pins.is_empty());
    assert!(closed.viewport_commands[&pin_id(1)].contains(&ViewportCommand::Close));
}

#[test]
fn locating_a_locked_pin_preserves_original_and_geometry() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.toggle_pin_lock(1, &ctx);
    let original = app.pins[&1].pixels.clone();
    let initial = app.pins[&1].initial;
    let texture = app.pins[&1].texture.id();
    let selected = ctx.run_logic(&egui::RawInput::default(), |ctx| app.select_pin(1, ctx));
    assert!(app.pins[&1].locked);
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    assert_eq!(app.pins[&1].initial, initial);
    assert_eq!(app.pins[&1].texture.id(), texture);
    assert!(
        selected
            .viewport_commands
            .values()
            .flatten()
            .all(|command| !matches!(
                command,
                ViewportCommand::Focus
                    | ViewportCommand::Visible(_)
                    | ViewportCommand::Minimized(_)
                    | ViewportCommand::OuterPosition(_)
                    | ViewportCommand::InnerSize(_)
            )),
        "list activation uses the platform adapter without rewriting geometry"
    );
    let count = app.pins.len();
    app.select_pin(999, &ctx);
    assert_eq!(
        app.pins.len(),
        count,
        "stale list entries cannot change pins"
    );
}

#[test]
fn pin_highlight_tracks_only_native_focus_and_does_not_expire() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let app = tiny_pin_app(&ctx);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin UI");
    for (time, focused, scale) in [
        (1.0, Some(true), 1.0),
        (1000.0, Some(true), 2.0),
        (1001.0, Some(false), 2.0),
        (1002.0, None, 1.5),
    ] {
        let mut input = egui::RawInput {
            viewport_id: pin_id(1),
            time: Some(time),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(100.0, 60.0),
            )),
            ..Default::default()
        };
        input.viewports.insert(
            pin_id(1),
            egui::ViewportInfo {
                focused,
                native_pixels_per_point: Some(scale),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| {
            assert_eq!(pin_focused(ui.ctx(), 1), focused == Some(true));
            assert!(!pin_focused(ui.ctx(), 999));
            paint(ui);
        });
        output.textures_delta.clear();
        let highlighted = output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == ui::style::PIN.highlight && rect.stroke.width == ui::style::PIN.highlight_stroke_pixels / scale)
        });
        assert_eq!(highlighted, focused == Some(true));
        assert!(
            output
                .viewport_output
                .values()
                .all(|viewport| viewport.commands.iter().all(|command| !matches!(
                    command,
                    ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
                )))
        );
    }
}

#[test]
fn explicit_zoom_uses_observed_dpi_and_preserves_source_and_initial_geometry() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx.set_zoom_factor(2.0);
    let mut app = tiny_pin_app(&ctx);
    let initial = app.pins[&1].initial;
    let pixels = app.pins[&1].pixels.clone();
    let texture = app.pins[&1].texture.id();
    for (command, expected) in [
        (pin::Zoom::In, egui::vec2(125.0, 75.0)),
        (pin::Zoom::Out, egui::vec2(80.0, 48.0)),
        (pin::Zoom::Original, egui::vec2(0.75, 1.25)),
    ] {
        let mut input = egui::RawInput::default();
        input.viewports.insert(
            pin_id(1),
            egui::ViewportInfo {
                inner_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(-100.0, 37.0),
                    egui::vec2(100.0, 60.0),
                )),
                native_pixels_per_point: Some(2.0),
                ..Default::default()
            },
        );
        let output = ctx.run_logic(&input, |ctx| app.zoom_pin(1, &[command], None, ctx));
        assert_eq!(
            output.viewport_commands[&pin_id(1)],
            [ViewportCommand::InnerSize(expected)]
        );
    }
    assert_eq!(app.pins[&1].initial, initial);
    assert!(Arc::ptr_eq(&pixels, &app.pins[&1].pixels));
    assert_eq!(app.pins[&1].texture.id(), texture);
    let mut input = egui::RawInput::default();
    input.viewports.insert(
        pin_id(1),
        egui::ViewportInfo {
            native_pixels_per_point: Some(2.0),
            ..Default::default()
        },
    );
    let output = ctx.run_logic(&input, |ctx| {
        app.zoom_pin(1, &[pin::Zoom::Original], None, ctx)
    });
    assert_eq!(
        output.viewport_commands[&pin_id(1)],
        [ViewportCommand::InnerSize(egui::vec2(0.75, 1.25))]
    );
    let absent = ctx.run_logic(&input, |ctx| app.zoom_pin(1, &[pin::Zoom::In], None, ctx));
    assert!(
        !absent.viewport_commands.contains_key(&pin_id(1)),
        "missing observed size must not fall back to initial geometry"
    );
    app.pins.get_mut(&1).expect("pin").locked = true;
    app.sender
        .send(Event::ZoomPin(
            1,
            vec![pin::Zoom::In],
            Some((egui::vec2(200.0, 120.0), 2.0)),
        ))
        .expect("queued zoom");
    let locked = ctx.run_logic(&input, |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        app.tray_action(crate::tray::Action::RestorePin(1), ctx);
    });
    assert!(
        locked
            .viewport_commands
            .values()
            .flatten()
            .all(|command| !matches!(
                command,
                ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
            ))
    );
}

#[test]
fn queued_zoom_keys_accumulate_in_order_and_request_one_native_size() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    let original = Arc::new(ColorImage::filled([200, 120], egui::Color32::BLACK));
    app.pins.get_mut(&1).expect("pin").pixels = original.clone();
    for (commands, expected) in [
        (
            vec![pin::Zoom::In, pin::Zoom::In],
            egui::vec2(156.25, 93.75),
        ),
        (vec![pin::Zoom::In, pin::Zoom::Out], egui::vec2(100.0, 60.0)),
        (
            vec![pin::Zoom::Original, pin::Zoom::In],
            egui::vec2(250.0, 150.0),
        ),
    ] {
        app.sender
            .send(Event::ZoomPin(
                1,
                commands,
                Some((egui::vec2(100.0, 60.0), 1.0)),
            ))
            .expect("zoom keys");
        let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        });
        let sizes = output.viewport_commands[&pin_id(1)]
            .iter()
            .filter_map(|command| match command {
                ViewportCommand::InnerSize(size) => Some(*size),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(sizes, [expected]);
        assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
        assert!(
            output.viewport_commands[&pin_id(1)]
                .iter()
                .all(|command| !matches!(command, ViewportCommand::OuterPosition(_)))
        );
    }
}

#[test]
fn keyboard_movement_uses_observed_outer_origin_and_respects_locks() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx.set_zoom_factor(2.0);
    let mut app = tiny_pin_app(&ctx);
    let initial = app.pins[&1].initial;
    let original = app.pins[&1].pixels.clone();
    let mut input = egui::RawInput::default();
    input.viewports.insert(
        pin_id(1),
        egui::ViewportInfo {
            outer_rect: Some(egui::Rect::from_min_size(
                egui::pos2(-50.0, 18.5),
                egui::vec2(40.0, 20.0),
            )),
            inner_rect: Some(egui::Rect::from_min_size(
                egui::pos2(500.0, 500.0),
                egui::vec2(30.0, 10.0),
            )),
            native_pixels_per_point: Some(2.0),
            ..Default::default()
        },
    );
    let output = ctx.run_logic(&input, |ctx| app.move_pin(1, [2, -3], None, ctx));
    let step = if cfg!(target_os = "macos") { 1.0 } else { 0.5 };
    assert_eq!(
        output.viewport_commands[&pin_id(1)],
        [ViewportCommand::OuterPosition(egui::pos2(
            (-100.0 + 2.0 * step) / 2.0,
            (37.0 - 3.0 * step) / 2.0
        ))]
    );
    assert_eq!(app.pins[&1].initial, initial);
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    input
        .viewports
        .get_mut(&pin_id(1))
        .expect("viewport")
        .outer_rect = None;
    let absent = ctx.run_logic(&input, |ctx| app.move_pin(1, [1, 0], None, ctx));
    assert!(
        !absent.viewport_commands.contains_key(&pin_id(1)),
        "do not guess a native outer origin from content or initial geometry"
    );
    app.sender
        .send(Event::MovePin(
            1,
            [1, 0],
            Some((egui::pos2(-100.0, 37.0), 2.0)),
        ))
        .expect("queued before lock");
    app.pins.get_mut(&1).expect("pin").locked = true;
    let locked = ctx.run_logic(&input, |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest())
    });
    assert!(
        locked
            .viewport_commands
            .values()
            .flatten()
            .all(|command| !matches!(
                command,
                ViewportCommand::OuterPosition(_) | ViewportCommand::InnerSize(_)
            ))
    );
}

#[test]
fn pin_dpi_and_ui_zoom_never_impose_an_application_minimum_size() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx.set_zoom_factor(1.25);
    let app = tiny_pin_app(&ctx);
    let initial = app.pins[&1].initial;
    for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
        let mut input = egui::RawInput::default();
        input.viewports.insert(
            pin_id(1),
            egui::ViewportInfo {
                native_pixels_per_point: Some(scale),
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| app.show_pins(ui.ctx()));
        output.textures_delta.clear();
        let viewport = &output.viewport_output[&pin_id(1)];
        assert!(viewport.builder.min_inner_size.is_none());
        assert_eq!(viewport.builder.position, Some(initial.min));
        assert_eq!(viewport.builder.inner_size, Some(initial.size()));
        assert!(viewport.commands.iter().all(|command| !matches!(
            command,
            ViewportCommand::InnerSize(_) | ViewportCommand::OuterPosition(_)
        )));
    }
}

#[test]
fn home_zoom_hint_uses_current_native_content_size_instead_of_source_or_initial_size() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.pins[&1].ready.store(true, Ordering::Release);
    for (display_size, focused, locked, expected) in [
        (None, true, false, false),
        (Some(egui::vec2(320.0, 240.0)), true, false, false),
        (Some(egui::vec2(1.0, 240.0)), true, false, true),
        (Some(egui::vec2(240.0, 1.0)), true, false, true),
        (
            Some(egui::Vec2::splat(pin::METRICS.button)),
            true,
            false,
            false,
        ),
        (Some(egui::Vec2::splat(1.0)), false, false, false),
        (Some(egui::Vec2::splat(1.0)), true, true, false),
    ] {
        app.pins.get_mut(&1).expect("pin").locked = locked;
        for _ in 0..2 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(390.0, 800.0),
                )),
                ..Default::default()
            };
            input.viewports.insert(
                pin_id(1),
                egui::ViewportInfo {
                    inner_rect: display_size
                        .map(|size| egui::Rect::from_min_size(egui::pos2(100.0, 100.0), size)),
                    focused: Some(focused),
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |ui| {
                app.show_pins(ui.ctx());
                eframe::App::ui(&mut app, ui, &mut eframe::Frame::_new_kittest());
            });
            output.textures_delta.clear();
            assert_eq!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == Locale::Chinese.text(Text::PinZoomHint))), expected);
        }
    }
}

#[test]
fn pin_size_classification_wakes_home_only_on_changes_and_is_released_on_close() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.pins[&1].ready.store(true, Ordering::Release);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let paint = setup.viewport_output[&pin_id(1)]
        .viewport_ui_cb
        .as_ref()
        .expect("pin callback")
        .clone();
    let wakes = Arc::new(AtomicBool::new(false));
    let observed_wakes = wakes.clone();
    ctx.set_request_repaint_callback(move |request| {
        if request.viewport_id == ViewportId::ROOT {
            observed_wakes.store(true, Ordering::Relaxed);
        }
    });
    for (size, small, changed) in [
        (egui::vec2(100.0, 100.0), false, true),
        (egui::vec2(120.0, 100.0), false, false),
        (egui::vec2(1.0, 100.0), true, true),
        (egui::vec2(2.0, 100.0), true, false),
        (egui::vec2(100.0, 100.0), false, true),
    ] {
        // Consume previously queued root paints before observing this child frame.
        for _ in 0..3 {
            let mut drained = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
            drained.textures_delta.clear();
        }
        wakes.store(false, Ordering::Relaxed);
        let mut input = egui::RawInput {
            viewport_id: pin_id(1),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        input
            .viewports
            .insert(pin_id(1), egui::ViewportInfo::default());
        let mut output = ctx.run_ui(input, |ui| paint(ui));
        output.textures_delta.clear();
        assert_eq!(wakes.load(Ordering::Relaxed), changed);
        assert_eq!(
            ctx.data_mut(|data| data.get_temp::<bool>(pin_hint_state_id(1))),
            Some(small)
        );
    }
    app.close_pin(1, &ctx);
    assert!(
        ctx.data_mut(|data| data.get_temp::<bool>(pin_hint_state_id(1)))
            .is_none()
    );
}

fn home_frame(
    app: &mut Snip2Pin,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            // The native host declares live pin viewports during App::logic
            // before painting home. Keep that lifecycle in this headless frame.
            app.show_pins(ui.ctx());
            eframe::App::ui(app, ui, &mut eframe::Frame::_new_kittest());
        },
    );
    output.textures_delta.clear();
    output
}

#[test]
#[cfg(target_os = "macos")]
fn macos_background_startup_waits_for_system_theme_before_overriding_the_bundle_icon() {
    let ctx = egui::Context::default();
    let mut app = app();
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| app.update_icon(ctx));
    assert_eq!(
        app.applied_icon, None,
        "unknown system theme must not briefly select the light icon"
    );
    assert!(app.icon_data.is_none());
    let mut output = ctx.run_ui(
        egui::RawInput {
            system_theme: Some(egui::Theme::Dark),
            ..Default::default()
        },
        |ui| app.update_icon(ui.ctx()),
    );
    output.textures_delta.clear();
    assert_eq!(app.applied_icon, Some(Variant::Metal));
}

#[test]
fn editing_pin_names_preserves_identity_originals_and_geometry_when_other_rows_close() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    add_second_test_pin(&mut app);
    app.toggle_pin_lock(1, &ctx);
    let pixels = app.pins[&1].pixels.clone();
    let initial = app.pins[&1].initial;
    let texture = app.pins[&1].texture.id();
    let session = app.pins[&1].session;
    let _ = home_frame(&mut app, &ctx, vec![]);
    let output = home_frame(&mut app, &ctx, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text() == Text::PinLabel.with(1).render(Locale::Chinese) =>
            {
                Some(
                    shape
                        .shape
                        .visual_bounding_rect()
                        .intersect(shape.clip_rect)
                        .center(),
                )
            }
            _ => None,
        })
        .expect("editable pin name");
    let _ = home_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
    for pressed in [true, false, true, false] {
        let _ = home_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    let output = home_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text("参考 Reference".into())],
    );
    assert_eq!(app.pins[&1].name, "参考 Reference");
    assert!(app.pins[&2].name.is_empty());
    assert!(ctx.egui_wants_keyboard_input());
    let editor = ctx.memory(|memory| memory.focused()).expect("name editor");
    assert!(egui::text_edit::TextEditState::load(&ctx, editor).is_some());
    assert!(output.platform_output.commands.is_empty());
    app.close_pin(2, &ctx);
    let _ = home_frame(&mut app, &ctx, vec![egui::Event::Text(" 2".into())]);
    assert_eq!(app.pins[&1].name, "参考 Reference 2");
    assert!(app.pins[&1].locked);
    assert!(Arc::ptr_eq(&pixels, &app.pins[&1].pixels));
    assert_eq!(app.pins[&1].initial, initial);
    assert_eq!(app.pins[&1].texture.id(), texture);
    assert_eq!(app.pins[&1].session, session);
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.capture_worker.is_none());
    app.close_pin(1, &ctx);
    assert!(egui::text_edit::TextEditState::load(&ctx, editor).is_some());
    app.toggle_pin_lock(1, &ctx);
    app.close_pin(1, &ctx);
    assert!(egui::text_edit::TextEditState::load(&ctx, editor).is_none());
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(editor));
}

#[test]
fn quitting_with_locked_pins_requires_confirmation_and_cancel_preserves_every_pin() {
    for (confirm, close_home) in [(false, false), (true, false), (false, true), (true, true)] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        add_second_test_pin(&mut app);
        app.toggle_pin_lock(1, &ctx);
        // Store only a command description. This test never spawns a process.
        *app.relaunch.lock().expect("pending relaunch") =
            Some(std::process::Command::new("generated-relaunch-fixture"));
        let original = Arc::downgrade(&app.pins[&1].pixels);
        let requested = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            if close_home {
                assert!(
                    app.tray.is_none(),
                    "exercise closing home without a tray backend"
                );
                app.close_home(ctx);
            } else {
                app.tray_action(crate::tray::Action::Quit, ctx);
            }
        });
        assert!(app.quit_confirmation.is_some());
        assert!(
            matches!(
                app.quit_confirmation
                    .as_ref()
                    .expect("pending quit")
                    .presentation,
                QuitPresentation::Fallback
            ),
            "demo must retain its generated GUI confirmation without opening a native dialog"
        );
        assert!(!app.exiting);
        assert_eq!(app.pins.len(), 2);
        let root_commands = &requested.viewport_commands[&ViewportId::ROOT];
        assert_eq!(
            root_commands.contains(&ViewportCommand::CancelClose),
            close_home
        );
        assert!(
            root_commands.contains(&ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop
            )),
            "quit confirmation must be visible above locked pins"
        );
        assert!(
            requested
                .viewport_commands
                .values()
                .flatten()
                .all(|command| *command != ViewportCommand::Close)
        );
        let _ = home_frame(&mut app, &ctx, vec![]);
        let output = home_frame(&mut app, &ctx, vec![]);
        let key = if confirm {
            Text::QuitConfirm
        } else {
            Text::Cancel
        };
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == Locale::Chinese.text(key) => {
                    Some(shape.shape.visual_bounding_rect().center())
                }
                _ => None,
            })
            .expect("confirmation has an actionable button");
        let _ = home_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
        let event = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = home_frame(&mut app, &ctx, vec![event(true)]);
        let clicked = home_frame(&mut app, &ctx, vec![event(false)]);
        assert!(app.quit_confirmation.is_none());
        assert_eq!(app.exiting, confirm);
        assert_eq!(
            app.relaunch.lock().expect("pending relaunch").is_some(),
            confirm,
            "cancel must discard a pending restart; confirmed quit preserves it for the host"
        );
        if confirm {
            assert!(app.pins.is_empty());
            assert!(
                original.upgrade().is_none(),
                "confirmed quit must release all in-memory originals"
            );
            for id in [pin_id(1), pin_id(2), ViewportId::ROOT] {
                assert!(
                    clicked.viewport_output[&id]
                        .commands
                        .contains(&ViewportCommand::Close)
                );
            }
        } else {
            assert_eq!(app.pins.len(), 2);
            assert!(app.pins[&1].locked);
            assert!(original.upgrade().is_some());
            assert!(
                clicked.viewport_output[&ViewportId::ROOT]
                    .commands
                    .contains(&ViewportCommand::WindowLevel(egui::WindowLevel::Normal)),
                "cancel must restore Home's normal level"
            );
            assert!(
                clicked
                    .viewport_output
                    .values()
                    .flat_map(|output| &output.commands)
                    .all(|command| *command != ViewportCommand::Close)
            );
        }
    }
}

#[test]
fn closing_home_without_a_tray_or_locked_pins_quits_and_releases_images() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    add_second_test_pin(&mut app);
    let original = Arc::downgrade(&app.pins[&1].pixels);
    assert!(app.tray.is_none());
    assert!(app.pins.values().all(|pin| !pin.locked));
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| app.close_home(ctx));
    assert!(app.exiting);
    assert!(app.quit_confirmation.is_none());
    assert!(app.pins.is_empty());
    assert!(original.upgrade().is_none());
    for id in [pin_id(1), pin_id(2), ViewportId::ROOT] {
        assert!(output.viewport_commands[&id].contains(&ViewportCommand::Close));
    }
    let root_commands = &output.viewport_commands[&ViewportId::ROOT];
    assert!(!root_commands.contains(&ViewportCommand::CancelClose));
    assert!(!root_commands.contains(&ViewportCommand::WindowLevel(
        egui::WindowLevel::AlwaysOnTop
    )));
}

#[test]
fn asynchronous_quit_results_confirm_or_cancel_only_the_current_request() {
    for confirmed in [false, true] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        add_second_test_pin(&mut app);
        app.toggle_pin_lock(1, &ctx);
        let original = Arc::downgrade(&app.pins[&1].pixels);
        let initial = app.pins[&1].initial;
        *app.relaunch.lock().expect("pending relaunch") =
            Some(std::process::Command::new("generated-relaunch-fixture"));
        let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            app.tray_action(crate::tray::Action::Quit, ctx);
        });
        let id = app.quit_confirmation.as_ref().expect("pending quit").id;
        app.sender
            .send(Event::QuitDecision(id, Ok(confirmed)))
            .expect("asynchronous decision");
        let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        });
        assert!(app.quit_confirmation.is_none());
        assert_eq!(app.exiting, confirmed);
        assert_eq!(
            app.relaunch.lock().expect("pending relaunch").is_some(),
            confirmed
        );
        assert!(
            !output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyImage(_))),
            "quit decisions must not write image data to the clipboard"
        );
        if confirmed {
            assert!(app.pins.is_empty());
            assert!(original.upgrade().is_none());
            for viewport in [pin_id(1), pin_id(2), ViewportId::ROOT] {
                assert!(output.viewport_commands[&viewport].contains(&ViewportCommand::Close));
            }
        } else {
            assert_eq!(app.pins.len(), 2);
            assert!(app.pins[&1].locked);
            assert_eq!(app.pins[&1].initial, initial);
            assert!(Arc::ptr_eq(
                &original.upgrade().expect("retained original"),
                &app.pins[&1].pixels
            ));
            assert!(
                output
                    .viewport_commands
                    .get(&ViewportId::ROOT)
                    .is_some_and(|commands| commands
                        .contains(&ViewportCommand::WindowLevel(egui::WindowLevel::Normal)))
            );
            assert!(
                output
                    .viewport_commands
                    .values()
                    .flatten()
                    .all(|command| *command != ViewportCommand::Close)
            );
        }
    }
}

#[test]
fn repeated_quit_reuses_its_request_and_late_results_cannot_resolve_a_new_prompt() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.toggle_pin_lock(1, &ctx);
    let original = app.pins[&1].pixels.clone();
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx)
    });
    let first = app.quit_confirmation.as_ref().expect("first prompt").id;
    let cancelled = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.quit_decision(first, Ok(false), ctx)
    });
    assert!(app.quit_confirmation.is_none());
    assert!(
        cancelled
            .viewport_commands
            .values()
            .flatten()
            .all(|command| *command != ViewportCommand::Close)
    );
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx)
    });
    let second = app.quit_confirmation.as_ref().expect("second prompt").id;
    assert_ne!(first, second);
    let next_request = app.next_quit_request;
    let repeated = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx);
        app.tray_action(crate::tray::Action::Quit, ctx);
    });
    assert_eq!(
        app.quit_confirmation.as_ref().expect("same prompt").id,
        second
    );
    assert_eq!(app.next_quit_request, next_request);
    assert!(
        repeated.viewport_commands.is_empty(),
        "repeated quit must not reopen or refocus its prompt"
    );
    for confirmed in [true, false] {
        app.sender
            .send(Event::QuitDecision(first, Ok(confirmed)))
            .expect("late decision");
    }
    let stale = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert_eq!(
        app.quit_confirmation
            .as_ref()
            .expect("new prompt survives")
            .id,
        second
    );
    assert!(!app.exiting);
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    assert!(
        stale
            .viewport_commands
            .values()
            .flatten()
            .all(|command| *command != ViewportCommand::Close)
    );
}

#[test]
fn pending_quit_keeps_its_host_after_the_last_pin_is_unlocked() {
    for confirmed in [false, true] {
        let ctx = egui::Context::default();
        let mut app = tiny_pin_app(&ctx);
        app.toggle_pin_lock(1, &ctx);
        let original = Arc::downgrade(&app.pins[&1].pixels);
        let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            app.tray_action(crate::tray::Action::Quit, ctx);
            app.toggle_pin_lock(1, ctx);
        });
        let id = app.quit_confirmation.as_ref().expect("pending quit").id;
        assert!(app.tray.is_none());
        assert!(!app.pins[&1].locked);
        let output = ctx.run_logic(&egui::RawInput::default(), |ctx| app.close_home(ctx));
        assert!(
            output
                .viewport_commands
                .get(&ViewportId::ROOT)
                .is_some_and(|commands| { commands.contains(&ViewportCommand::CancelClose) }),
            "a pending confirmation must retain its host even if lock state changes"
        );
        assert!(!app.exiting);
        assert_eq!(app.quit_confirmation.as_ref().expect("same request").id, id);
        assert!(original.upgrade().is_some());
        app.quit_decision(id, Ok(confirmed), &ctx);
        assert!(app.quit_confirmation.is_none());
        assert_eq!(app.exiting, confirmed);
        assert_eq!(original.upgrade().is_none(), confirmed);
    }
}

#[test]
fn english_quit_fallback_ignores_enter_and_escape_preserves_pins() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    app.preferences.language = Language::English;
    app.toggle_pin_lock(1, &ctx);
    let original = app.pins[&1].pixels.clone();
    *app.relaunch.lock().expect("pending relaunch") =
        Some(std::process::Command::new("generated-relaunch-fixture"));
    app.tray_action(crate::tray::Action::Quit, &ctx);
    let _ = home_frame(&mut app, &ctx, vec![]);
    let output = home_frame(&mut app, &ctx, vec![]);
    for key in [
        Text::QuitLockedTitle,
        Text::QuitLockedHelp,
        Text::Cancel,
        Text::QuitConfirm,
    ] {
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == Locale::English.text(key))
        }), "missing English confirmation text: {key:?}");
    }
    for key in [egui::Key::Enter, egui::Key::Escape] {
        let _ = home_frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!app.exiting);
        assert_eq!(app.quit_confirmation.is_some(), key == egui::Key::Enter);
        assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    }
    assert!(app.relaunch.lock().expect("pending relaunch").is_none());
}

#[test]
fn confirmed_quit_ignores_a_later_capture_event_in_the_same_batch() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    app.toggle_pin_lock(1, &ctx);
    app.tray_action(crate::tray::Action::Quit, &ctx);
    let id = app.quit_confirmation.as_ref().expect("pending quit").id;
    app.sender
        .send(Event::QuitDecision(id, Ok(true)))
        .expect("confirm");
    app.sender.send(Event::Capture).expect("late shortcut");
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert!(app.exiting);
    assert!(app.pins.is_empty());
    assert!(matches!(app.phase, Phase::Idle));
    assert!(
        app.capture_worker.is_none(),
        "exiting must not start capture work"
    );
}

#[test]
fn quitting_during_capture_discards_late_results_and_can_resume_after_cancel() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    app.toggle_pin_lock(1, &ctx);
    let original = app.pins[&1].pixels.clone();
    let capture = app.gate.begin().expect("pending generated capture");
    app.phase = Phase::Capturing;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx);
        app.accept_capture(capture, Ok(vec![capture::demo()]), None, ctx);
        app.capture_command(ctx);
    });
    assert!(matches!(app.phase, Phase::Idle));
    assert!(app.capture_worker.is_none());
    assert!(app.layers.values().all(|layer| layer.selection.is_none()));
    assert!(output.platform_output.commands.is_empty());
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    let id = app.quit_confirmation.as_ref().expect("pending quit").id;
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.quit_decision(id, Ok(false), ctx);
        app.capture_command(ctx);
    });
    assert!(!app.exiting);
    assert!(matches!(app.phase, Phase::Capturing));
    assert!(app.capture_worker.is_some());
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
}

#[test]
fn native_quit_without_a_host_handle_waits_one_step_then_uses_gui_fallback() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    // Set state before opting into the native presentation path. No native pin
    // adapter is called, and the test frame never owns a real host window.
    app.pins.get_mut(&1).expect("pin").locked = true;
    app.demo = false;
    let frame = eframe::Frame::_new_kittest();
    assert!(
        frame.window_handle().is_err(),
        "the test must never be able to open a native dialog"
    );
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.tray_action(crate::tray::Action::Quit, ctx)
    });
    let id = app.quit_confirmation.as_ref().expect("pending quit").id;
    assert!(matches!(
        app.quit_confirmation
            .as_ref()
            .expect("pending quit")
            .presentation,
        QuitPresentation::AwaitingHost
    ));
    for expected_ready in [true, false, false] {
        let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            app.prepare_quit_prompt(ctx, &frame)
        });
        let pending = app.quit_confirmation.as_ref().expect("pending quit");
        assert_eq!(pending.id, id);
        assert_eq!(
            matches!(pending.presentation, QuitPresentation::Ready),
            expected_ready
        );
        if !expected_ready {
            assert!(matches!(pending.presentation, QuitPresentation::Fallback));
        }
        assert!(output.viewport_commands.is_empty());
        assert!(output.platform_output.commands.is_empty());
        assert!(
            app.events.try_recv().is_err(),
            "no native callback can run without a host"
        );
        assert!(!app.exiting);
        assert!(app.pins.contains_key(&1));
    }
}

#[test]
fn native_quit_waits_for_a_new_host_frame_after_a_discarded_ui_pass() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    app.pins.get_mut(&1).expect("pin").locked = true;
    app.demo = false;
    let frame = eframe::Frame::_new_kittest();
    assert!(frame.window_handle().is_err());
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        if ui.ctx().current_pass_index() == 0 {
            app.tray_action(crate::tray::Action::Quit, ui.ctx());
            ui.ctx()
                .request_discard("exercise native prompt host ordering");
        }
        app.prepare_quit_prompt(ui.ctx(), &frame);
    });
    output.textures_delta.clear();
    assert_eq!(output.platform_output.num_completed_passes, 2);
    assert!(
        matches!(
            app.quit_confirmation
                .as_ref()
                .expect("pending")
                .presentation,
            QuitPresentation::Ready
        ),
        "a second pass cannot start a native prompt before host commands apply"
    );
    let mut next = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.prepare_quit_prompt(ui.ctx(), &frame);
    });
    next.textures_delta.clear();
    assert!(matches!(
        app.quit_confirmation
            .as_ref()
            .expect("fallback")
            .presentation,
        QuitPresentation::Fallback
    ));
    assert!(!app.exiting);
    assert!(app.pins[&1].locked);
}

#[test]
fn failed_native_quit_presentation_falls_back_without_losing_pins() {
    let ctx = egui::Context::default();
    let mut app = tiny_pin_app(&ctx);
    app.toggle_pin_lock(1, &ctx);
    let original = app.pins[&1].pixels.clone();
    app.tray_action(crate::tray::Action::Quit, &ctx);
    let id = app.quit_confirmation.as_ref().expect("pending").id;
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        app.quit_decision(id, Err(Text::QuitDialogUnavailable.into()), ctx);
    });
    let pending = app
        .quit_confirmation
        .as_ref()
        .expect("fallback keeps request");
    assert_eq!(pending.id, id);
    assert!(matches!(pending.presentation, QuitPresentation::Fallback));
    assert!(!app.exiting);
    assert!(Arc::ptr_eq(&original, &app.pins[&1].pixels));
    assert!(
        output
            .viewport_commands
            .values()
            .flatten()
            .all(|command| *command != ViewportCommand::Close)
    );
    app.quit_decision(id, Ok(false), &ctx);
    assert!(app.quit_confirmation.is_none());
    assert!(app.pins[&1].locked);
}

#[test]
#[cfg(target_os = "macos")]
fn pin_initializes_hidden_and_becomes_visible_without_a_first_paint() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let app = tiny_pin_app(&ctx);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let viewport = &setup.viewport_output[&pin_id(1)];
    assert_eq!(viewport.builder.visible, Some(false));
    let position = viewport.builder.position;
    let size = viewport.builder.inner_size.expect("initial size");
    let paint = viewport.viewport_ui_cb.as_ref().expect("pin UI");
    let mut initialized = egui::RawInput::default();
    initialized.viewports.insert(
        pin_id(1),
        egui::ViewportInfo {
            inner_rect: Some(egui::Rect::from_min_size(
                position.expect("initial position"),
                size,
            )),
            occluded: Some(true),
            ..Default::default()
        },
    );
    let mut shown = ctx.run_ui(initialized, |ui| {
        app.show_pins(ui.ctx());
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx()
                .request_discard("visibility must survive a discarded root pass");
        }
    });
    shown.textures_delta.clear();
    assert!(!app.pins[&1].ready.load(Ordering::Acquire));
    assert_eq!(
        shown.viewport_output[&pin_id(1)]
            .commands
            .iter()
            .filter(|command| matches!(command, ViewportCommand::Visible(true)))
            .count(),
        1
    );
    let mut input = egui::RawInput {
        viewport_id: pin_id(1),
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    input
        .viewports
        .insert(pin_id(1), egui::ViewportInfo::default());
    let mut painted = ctx.run_ui(input, |ui| {
        paint(ui);
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx()
                .request_discard("first pin paint can run multiple passes");
        }
    });
    painted.textures_delta.clear();
    assert!(painted.shapes.iter().any(|shape|
        matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == app.pins[&1].texture.id())));
    assert_eq!(
        app.events
            .try_iter()
            .filter(|event| matches!(event, Event::PinReady(_, 1)))
            .count(),
        1
    );
    for repeat in [true, false] {
        let mut shown = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.show_pins(ui.ctx());
            if repeat && ui.ctx().current_pass_index() == 0 {
                ui.ctx()
                    .request_discard("visibility must survive a discarded root pass");
            }
        });
        shown.textures_delta.clear();
        let viewport = &shown.viewport_output[&pin_id(1)];
        assert_eq!(viewport.builder.visible, Some(false));
        assert_eq!(viewport.builder.position, position);
        assert_eq!(viewport.builder.inner_size, Some(size));
        assert!(!viewport.commands.iter().any(|command| matches!(
            command,
            ViewportCommand::OuterPosition(_)
                | ViewportCommand::InnerSize(_)
                | ViewportCommand::Visible(_)
        )));
    }
}

#[test]
#[cfg(target_os = "macos")]
fn initialized_pin_can_be_shown_when_all_windows_are_occluded() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let app = tiny_pin_app(&ctx);
    let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
    setup.textures_delta.clear();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&ViewportId::ROOT)
        .expect("root")
        .occluded = Some(true);
    input.viewports.insert(
        pin_id(1),
        egui::ViewportInfo {
            inner_rect: Some(app.pins[&1].initial),
            occluded: Some(true),
            ..Default::default()
        },
    );
    let output = ctx.run_logic(&input, |ctx| app.show_pins(ctx));
    assert!(matches!(
        output.viewport_commands[&pin_id(1)].as_slice(),
        [ViewportCommand::Visible(true)]
    ));
    assert!(!app.pins[&1].ready.load(Ordering::Acquire));
    let output = ctx.run_logic(&input, |ctx| app.show_pins(ctx));
    assert!(!output.viewport_commands.contains_key(&pin_id(1)));
}

#[test]
fn tiny_pin_opens_an_independent_menu_only_on_request_without_resizing_the_image() {
    for zoom in [1.0, 1.25, 2.0] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        ctx.set_zoom_factor(zoom);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
        warmup.textures_delta.clear();
        let mut app = tiny_pin_app(&ctx);
        let initial = app.pins[&1].initial;
        let original = app.pins[&1].pixels.clone();
        let mut before = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.show_pins(ui.ctx());
            app.show_pin_menu(ui.ctx());
        });
        before.textures_delta.clear();
        assert!(!before.viewport_output.contains_key(&pin_menu_id()));
        assert_eq!(
            before.viewport_output[&pin_id(1)].builder.inner_size,
            Some(initial.size())
        );
        assert!(
            before.viewport_output[&pin_id(1)]
                .builder
                .min_inner_size
                .is_none()
        );
        let area = app.pins[&1].screen;
        let position = egui::pos2(
            area.right() / zoom - ui::pin::MENU_SIZE.x,
            area.bottom() / zoom - ui::pin::MENU_SIZE.y,
        );
        app.sender
            .send(Event::MenuPin(1, Some(position)))
            .expect("menu event");
        let mut opened = ctx.run_ui(egui::RawInput::default(), |ui| {
            eframe::App::logic(&mut app, ui.ctx(), &mut eframe::Frame::_new_kittest());
            app.show_pins(ui.ctx());
            app.show_pin_menu(ui.ctx());
        });
        opened.textures_delta.clear();
        assert_eq!(app.pin_menu.as_ref().expect("requested menu").pin_id, 1);
        let menu = &opened.viewport_output[&pin_menu_id()].builder;
        assert_eq!(menu.inner_size, Some(ui::pin::MENU_SIZE));
        assert_eq!(menu.resizable, Some(false));
        assert_eq!(menu.active, Some(true));
        let menu_rect =
            egui::Rect::from_min_size(menu.position.expect("anchored menu"), ui::pin::MENU_SIZE);
        let visible = egui::Rect::from_min_max(area.min / zoom, area.max / zoom);
        assert!(visible.contains_rect(menu_rect));
        assert_eq!(
            opened.viewport_output[&pin_id(1)].builder.inner_size,
            Some(initial.size())
        );
        assert_eq!(app.pins[&1].initial, initial);
        assert_eq!(app.pins[&1].pixels.as_ref(), original.as_ref());
        assert_eq!(original.size, [3, 5]);
    }
}

#[test]
fn right_click_menu_preserves_current_viewport_coordinates_after_dpi_changes() {
    for (capture_scale, current_scale) in [(1.0, 2.0), (2.0, 1.0)] {
        for zoom in [1.0, 1.25] {
            let ctx = egui::Context::default();
            ctx.set_embed_viewports(false);
            ctx.set_zoom_factor(zoom);
            let mut warmup = ctx.run_ui(egui::RawInput::default(), |_| {});
            warmup.textures_delta.clear();
            let mut app = tiny_pin_app(&ctx);
            let pin = app.pins.get_mut(&1).expect("pin");
            pin.display_scale = capture_scale;
            // Store work areas in the capture coordinate domain, as selection_action does.
            let area =
                egui::Rect::from_min_max(egui::pos2(-2000.0, -1000.0), egui::pos2(6000.0, 4000.0));
            pin.screen = area;
            pin.screen_areas = Arc::new(vec![area]);
            let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pins(ui.ctx()));
            setup.textures_delta.clear();
            let paint = setup.viewport_output[&pin_id(1)]
                .viewport_ui_cb
                .as_ref()
                .expect("pin callback");
            let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(3.0, 5.0));
            let point = bounds.center();
            let window = egui::Rect::from_min_size(egui::pos2(1100.0, 250.0) / zoom, bounds.size());
            let expected = window.min + point.to_vec2();
            for events in [
                vec![egui::Event::PointerMoved(point)],
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            ] {
                let mut input = egui::RawInput {
                    viewport_id: pin_id(1),
                    screen_rect: Some(bounds),
                    events,
                    ..Default::default()
                };
                input.viewports.insert(
                    pin_id(1),
                    egui::ViewportInfo {
                        native_pixels_per_point: Some(current_scale),
                        inner_rect: Some(window),
                        ..Default::default()
                    },
                );
                let mut output = ctx.run_ui(input, |ui| paint(ui));
                output.textures_delta.clear();
            }
            let requested = app
                .events
                .try_iter()
                .find_map(|event| match event {
                    Event::MenuPin(1, position) => Some(position),
                    _ => None,
                })
                .expect("real pin right-click must dispatch a menu request");
            assert_eq!(
                requested,
                Some(expected),
                "capture DPI={capture_scale}, current DPI={current_scale}, zoom={zoom}"
            );
            app.sender
                .send(Event::MenuPin(1, requested))
                .expect("forward inspected event");
            let mut opened = ctx.run_ui(egui::RawInput::default(), |ui| {
                eframe::App::logic(&mut app, ui.ctx(), &mut eframe::Frame::_new_kittest());
                app.show_pin_menu(ui.ctx());
            });
            opened.textures_delta.clear();
            assert_eq!(
                opened.viewport_output[&pin_menu_id()].builder.position,
                Some(expected)
            );
        }
    }
}

#[test]
fn independent_menu_closes_on_escape_focus_loss_or_native_close_without_closing_the_pin() {
    for reason in ["escape", "focus", "close"] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        app.open_pin_menu(1, None, &ctx);
        let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pin_menu(ui.ctx()));
        setup.textures_delta.clear();
        let paint = setup.viewport_output[&pin_menu_id()]
            .viewport_ui_cb
            .as_ref()
            .expect("menu callback");
        for (index, focused) in [true, reason != "focus"].into_iter().enumerate() {
            let closing = index == 1;
            let mut input = egui::RawInput {
                viewport_id: pin_menu_id(),
                focused,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    ui::pin::MENU_SIZE,
                )),
                events: if closing && reason == "escape" {
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }]
                } else {
                    vec![]
                },
                ..Default::default()
            };
            input.viewports.insert(
                pin_menu_id(),
                egui::ViewportInfo {
                    focused: Some(focused),
                    events: if closing && reason == "close" {
                        vec![egui::ViewportEvent::Close]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |ui| paint(ui));
            output.textures_delta.clear();
        }
        let closed = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        });
        assert!(app.pin_menu.is_none(), "menu must close on {reason}");
        assert!(app.pins.contains_key(&1));
        assert!(closed.viewport_commands[&pin_menu_id()].contains(&ViewportCommand::Close));
    }
}

#[test]
fn independent_menu_actions_copy_original_or_close_pin_and_release_the_menu() {
    for close in [false, true] {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = tiny_pin_app(&ctx);
        i18n::set_locale(&ctx, app.preferences.language.resolve(app.system_locale));
        let original = app.pins[&1].pixels.clone();
        app.open_pin_menu(1, None, &ctx);
        let mut setup = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pin_menu(ui.ctx()));
        setup.textures_delta.clear();
        let paint = setup.viewport_output[&pin_menu_id()]
            .viewport_ui_cb
            .as_ref()
            .expect("menu callback");
        let mut layout_input = egui::RawInput {
            viewport_id: pin_menu_id(),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                ui::pin::MENU_SIZE,
            )),
            ..Default::default()
        };
        layout_input
            .viewports
            .insert(pin_menu_id(), egui::ViewportInfo::default());
        let mut layout = ctx.run_ui(layout_input, |ui| paint(ui));
        layout.textures_delta.clear();
        let label = app
            .preferences
            .language
            .resolve(app.system_locale)
            .text(if close { Text::Close } else { Text::Copy });
        let point = layout
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .expect("menu label");
        let button = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(point)],
            vec![button(true)],
            vec![button(false)],
        ] {
            let mut input = egui::RawInput {
                viewport_id: pin_menu_id(),
                focused: true,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    ui::pin::MENU_SIZE,
                )),
                events,
                ..Default::default()
            };
            input.viewports.insert(
                pin_menu_id(),
                egui::ViewportInfo {
                    focused: Some(true),
                    ..Default::default()
                },
            );
            let mut menu = ctx.run_ui(input, |ui| paint(ui));
            menu.textures_delta.clear();
        }
        let output = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        });
        assert!(app.pin_menu.is_none());
        assert_eq!(app.pins.contains_key(&1), !close);
        assert!(
            output.viewport_commands[&pin_menu_id()]
                .iter()
                .filter(|command| **command == ViewportCommand::Close)
                .count()
                == 1
        );
        let copied = output
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyImage(pixels) => Some(pixels),
                _ => None,
            })
            .collect::<Vec<_>>();
        if close {
            assert!(copied.is_empty());
        } else {
            assert_eq!(copied, [original.as_ref()]);
        }
    }
}

#[test]
fn stale_menu_requests_cannot_create_a_popup_for_a_closed_pin() {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    let mut app = tiny_pin_app(&ctx);
    app.close_pin(1, &ctx);
    app.open_pin_menu(1, Some(egui::pos2(100.0, 100.0)), &ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show_pin_menu(ui.ctx()));
    output.textures_delta.clear();
    assert!(app.pin_menu.is_none());
    assert!(!output.viewport_output.contains_key(&pin_menu_id()));
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
            positioned: Arc::new(AtomicBool::new(false)),
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
