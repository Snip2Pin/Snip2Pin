use super::*;
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, Rect, Vec2};

#[test]
fn bundled_fonts_keep_emoji_names_drawable_without_system_fonts() {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let name = "Reference 😀💡";
        assert!(
            ui.fonts_mut(|fonts| { fonts.has_glyphs(&egui::FontId::proportional(15.0), name) })
        );
        ui.label(name);
    });
    output.textures_delta.clear();
}

#[test]
fn responsive_setting_controls_fit_and_select_in_both_languages() {
    for locale in [Locale::English, Locale::Chinese] {
        for width in [210.0, LAUNCHER.window[0]] {
            for scale in [1.0, 2.0] {
                for (label, selected, target, expected) in [
                    (
                        Text::Theme,
                        Theme::System.label(locale),
                        Theme::Dark.label(locale),
                        0,
                    ),
                    (
                        Text::Icon,
                        Icon::Auto.label(locale),
                        Icon::Metal.label(locale),
                        1,
                    ),
                    (
                        Text::Language,
                        Language::System.label(locale),
                        Language::Chinese.label(locale),
                        2,
                    ),
                ] {
                    let ctx = egui::Context::default();
                    ctx.global_style_mut(|style| {
                        for font in style.text_styles.values_mut() {
                            font.size *= scale;
                        }
                    });
                    let size = egui::vec2(width, 1200.0);
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ui| {
                            ui.set_width(width - 2.0 * f32::from(LAUNCHER.margin));
                            let response = setting_row(
                                ui,
                                locale.text(label),
                                LAUNCHER.control_width,
                                |ui, _| {
                                    egui::ComboBox::from_id_salt("hit-rect")
                                        .width(LAUNCHER.control_width.min(ui.available_width()))
                                        .truncate()
                                        .selected_text(selected)
                                        .show_ui(ui, |_| {})
                                        .response
                                },
                            );
                            assert!(
                                ui.clip_rect().contains_rect(response.rect),
                                "entire hit rectangle fits"
                            );
                            assert!(response.rect.right() <= ui.max_rect().right());
                        },
                    );
                    output.textures_delta.clear();
                    let render = |events| localized_frame(&ctx, size, events, false, locale);
                    let _ = render(vec![]);
                    let output = render(vec![]).0;
                    let label_bounds = visible_text(&output, locale.text(label));
                    let text = output
                        .shapes
                        .iter()
                        .filter_map(|shape| {
                            if let egui::Shape::Text(text) = &shape.shape
                                && text.galley.text() == selected
                            {
                                let rect = shape.shape.visual_bounding_rect();
                                (rect.center().y >= label_bounds.center().y - 1.0).then_some(rect)
                            } else {
                                None
                            }
                        })
                        .min_by(|a, b| a.min.y.total_cmp(&b.min.y))
                        .expect("setting selected text");
                    let (bounds, clip) = output
                        .shapes
                        .iter()
                        .filter_map(|shape| {
                            if let egui::Shape::Rect(rect) = &shape.shape
                                && rect.rect.contains_rect(text)
                                && rect.rect.height() < text.height() * 3.0
                            {
                                Some((rect.rect, shape.clip_rect))
                            } else {
                                None
                            }
                        })
                        .min_by(|a, b| a.0.area().total_cmp(&b.0.area()))
                        .expect("combo button frame");
                    assert!(
                        clip.contains_rect(bounds),
                        "full button must fit: {locale:?}, {width}, {scale}: {bounds:?} in {clip:?}"
                    );
                    if width == LAUNCHER.window[0] && scale == 1.0 {
                        assert!(
                            (label_bounds.center().y - bounds.center().y).abs() < bounds.height()
                        );
                    }
                    let click_at = |point| {
                        let _ = render(vec![Event::PointerMoved(point)]);
                        let event = |pressed| Event::PointerButton {
                            pos: point,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        };
                        let _ = render(vec![event(true)]);
                        render(vec![event(false)])
                    };
                    let _ = click_at(bounds.center());
                    let popup = render(vec![]).0;
                    let option = visible_text(&popup, target);
                    let action = click_at(option.center()).1;
                    assert!(
                        match expected {
                            0 => matches!(action, Some(Action::Appearance(Theme::Dark, _))),
                            1 => matches!(action, Some(Action::Appearance(_, Icon::Metal))),
                            _ => matches!(action, Some(Action::Language(Language::Chinese))),
                        },
                        "selection: {locale:?}, {width}, {scale}, {label:?}"
                    );
                }
            }
        }
    }
}

fn frame(
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<Event>,
    busy: bool,
) -> (FullOutput, Option<Action>) {
    localized_frame(ctx, size, events, busy, Locale::Chinese)
}

fn localized_frame(
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<Event>,
    busy: bool,
    locale: Locale,
) -> (FullOutput, Option<Action>) {
    frame_with_pins(ctx, size, events, busy, locale, &[])
}

fn frame_with_pins(
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<Event>,
    busy: bool,
    locale: Locale,
    pins: &[PinEntry<'_>],
) -> (FullOutput, Option<Action>) {
    let mut action = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| {
            action = show(
                ui,
                Model {
                    locale,
                    pins,
                    message: None,
                    global_shortcut: true,
                    preferences: Preferences::default(),
                    icon_texture: None,
                    recording: busy,
                    idle: !busy,
                    capturing: false,
                    demo: false,
                    startup: Some(false),
                    permission_required: false,
                },
            );
        },
    );
    output.textures_delta.clear();
    (output, action)
}

fn visible_text(output: &FullOutput, label: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == label
            {
                let rect = shape
                    .shape
                    .visual_bounding_rect()
                    .intersect(shape.clip_rect);
                return rect.is_positive().then_some(rect);
            }
            None
        })
        .unwrap_or_else(|| panic!("visible control: {label}"))
}

fn click(ctx: &egui::Context, size: Vec2, point: Pos2, busy: bool) -> (FullOutput, Option<Action>) {
    let _ = frame(ctx, size, vec![Event::PointerMoved(point)], busy);
    let event = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let _ = frame(ctx, size, vec![event(true)], busy);
    frame(ctx, size, vec![event(false)], busy)
}

#[test]
fn release_link_remains_usable_in_small_windows_and_during_capture() {
    for size in [Vec2::from(LAUNCHER.window), egui::vec2(280.0, 220.0)] {
        for dark in [false, true] {
            for busy in [false, true] {
                let ctx = egui::Context::default();
                ctx.set_visuals(if dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
                let _ = frame(&ctx, size, vec![], busy);
                let (output, _) = frame(&ctx, size, vec![], busy);
                let link = visible_text(&output, "版本与下载 ↗");
                assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(link));
                let (output, action) = click(&ctx, size, link.center(), busy);
                assert!(
                    action.is_none(),
                    "navigation cannot start a capture or change preferences"
                );
                assert!(
                    output
                        .platform_output
                        .commands
                        .iter()
                        .any(|command| matches!(
                            command, egui::OutputCommand::OpenUrl(url)
                            if url.url == concat!(env!("CARGO_PKG_REPOSITORY"), "/releases")
                        )),
                    "the system browser is requested without opening it in the test"
                );
            }
        }
    }
}

#[test]
fn empty_home_keeps_capture_shortcut_and_settings_reachable() {
    let ctx = egui::Context::default();
    let size = Vec2::from(LAUNCHER.window);
    let _ = frame(&ctx, size, vec![], false);
    let (output, _) = frame(&ctx, size, vec![], false);
    for label in ["主题", "图标", "开机自启"] {
        visible_text(&output, label);
    }
    let capture = visible_text(&output, "截图并置顶").center();
    assert!(matches!(
        click(&ctx, size, capture, false).1,
        Some(Action::Capture)
    ));
    let shortcut = visible_text(&output, "修改快捷键").center();
    assert!(matches!(
        click(&ctx, size, shortcut, false).1,
        Some(Action::RecordShortcut)
    ));
    assert!(click(&ctx, size, capture, true).1.is_none());
}

#[test]
fn capture_tooltip_wraps_completely_inside_the_viewport() {
    for width in [280.0, LAUNCHER.window[0], 600.0] {
        for font_size in [13.0, 20.0] {
            let ctx = egui::Context::default();
            ctx.global_style_mut(|style| {
                style.interaction.tooltip_delay = 0.0;
                style
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_size));
            });
            let size = egui::vec2(width, 600.0);
            let _ = frame(&ctx, size, vec![], false);
            let (output, _) = frame(&ctx, size, vec![], false);
            let capture = visible_text(&output, "截图并置顶").center();
            let _ = frame(&ctx, size, vec![Event::PointerMoved(capture)], false);
            let mut output = frame(&ctx, size, vec![], false).0;
            for _ in 0..3 {
                output = frame(&ctx, size, vec![], false).0;
            }
            let tooltip = output
                .shapes
                .iter()
                .find(|shape| {
                    matches!(&shape.shape,
                        egui::Shape::Text(text) if text.galley.text().starts_with("拖动框选")
                    )
                })
                .expect("hover must show the real capture tooltip");
            let egui::Shape::Text(text) = &tooltip.shape else {
                unreachable!()
            };
            assert!(text.galley.text().ends_with("悬停贴图可复制、锁定或关闭。"));
            let painted = tooltip.shape.visual_bounding_rect();
            assert!(
                Rect::from_min_size(Pos2::ZERO, size)
                    .expand(1.0)
                    .contains_rect(painted),
                "tooltip exceeds viewport: width={width}, font={font_size}, {painted:?}"
            );
            assert!(
                tooltip.clip_rect.expand(1.0).contains_rect(painted),
                "tooltip text is clipped: {painted:?} vs {:?}",
                tooltip.clip_rect
            );
        }
    }
}

#[test]
fn long_shortcut_wraps_without_losing_the_change_action() {
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(210.0, 100.0));
    for _ in 0..2 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                action_row(ui, "Ctrl + Alt + Shift + Command + F12", "修改快捷键", true);
            },
        );
        output.textures_delta.clear();
        let change = visible_text(&output, "修改快捷键");
        assert!(screen.contains_rect(change));
    }
}

#[test]
fn both_languages_keep_primary_actions_and_settings_reachable() {
    for locale in [Locale::English, Locale::Chinese] {
        let ctx = egui::Context::default();
        let size = Vec2::from(LAUNCHER.window);
        let _ = localized_frame(&ctx, size, vec![], false, locale);
        let output = localized_frame(&ctx, size, vec![], false, locale).0;
        for key in [
            Text::Capture,
            Text::ChangeShortcut,
            Text::Theme,
            Text::Icon,
            Text::Language,
            Text::Startup,
            Text::Downloads,
        ] {
            let bounds = visible_text(&output, locale.text(key));
            assert!(
                Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds),
                "{locale:?} {key:?}"
            );
        }
        let capture = visible_text(&output, locale.text(Text::Capture)).center();
        let event = |pressed| Event::PointerButton {
            pos: capture,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = localized_frame(
            &ctx,
            size,
            vec![Event::PointerMoved(capture), event(true)],
            false,
            locale,
        );
        assert!(matches!(
            localized_frame(&ctx, size, vec![event(false)], false, locale).1,
            Some(Action::Capture)
        ));
    }
}

#[test]
fn english_capture_tooltip_wraps_in_a_small_viewport() {
    let ctx = egui::Context::default();
    ctx.global_style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let size = egui::vec2(280.0, 600.0);
    let _ = localized_frame(&ctx, size, vec![], false, Locale::English);
    let output = localized_frame(&ctx, size, vec![], false, Locale::English).0;
    let point = visible_text(&output, "Capture & pin").center();
    let _ = localized_frame(
        &ctx,
        size,
        vec![Event::PointerMoved(point)],
        false,
        Locale::English,
    );
    for _ in 0..4 {
        let output = localized_frame(&ctx, size, vec![], false, Locale::English).0;
        if let Some(shape) = output.shapes.iter().find(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == Locale::English.text(Text::CaptureHelp))) {
            let painted = shape.shape.visual_bounding_rect();
            assert!(shape.clip_rect.expand(1.0).contains_rect(painted));
            assert!(Rect::from_min_size(Pos2::ZERO, size).expand(1.0).contains_rect(painted));
            return;
        }
    }
    panic!("English hover instructions must be complete");
}

#[test]
fn language_picker_emits_a_language_action() {
    let ctx = egui::Context::default();
    let size = egui::vec2(LAUNCHER.window[0], 650.0);
    let _ = localized_frame(&ctx, size, vec![], false, Locale::English);
    let output = localized_frame(&ctx, size, vec![], false, Locale::English).0;
    let language_row = visible_text(&output, "Language");
    let choice = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Follow system" => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .min_by(|a, b| {
            (a.y - language_row.center().y)
                .abs()
                .total_cmp(&(b.y - language_row.center().y).abs())
        })
        .expect("language dropdown");
    let click_at = |point: Pos2| {
        let _ = localized_frame(
            &ctx,
            size,
            vec![Event::PointerMoved(point)],
            false,
            Locale::English,
        );
        let mut result = None;
        let mut output = None;
        for pressed in [true, false] {
            let frame = localized_frame(
                &ctx,
                size,
                vec![Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
                false,
                Locale::English,
            );
            result = result.or(frame.1);
            output = Some(frame.0);
        }
        (output.expect("painted"), result)
    };
    let _ = click_at(choice);
    let _ = localized_frame(&ctx, size, vec![], false, Locale::English);
    let output = localized_frame(&ctx, size, vec![], false, Locale::English).0;
    let english = visible_text(&output, "English").center();
    assert!(matches!(
        click_at(english).1,
        Some(Action::Language(Language::English))
    ));
}

#[test]
fn pin_list_thumbnail_and_buttons_emit_actions_in_narrow_homes() {
    for width in [210.0, 280.0, LAUNCHER.window[0]] {
        for locale in [Locale::English, Locale::Chinese] {
            for locked in [false, true] {
                let ctx = egui::Context::default();
                let size = egui::vec2(width, LAUNCHER.window[1]);
                let texture = ctx.load_texture(
                    "generated-list-pin",
                    egui::ColorImage::filled([3, 5], egui::Color32::LIGHT_BLUE),
                    egui::TextureOptions::default(),
                );
                let pins = [PinEntry {
                    display_size: None,
                    id: 17,
                    name: "",
                    texture: &texture,
                    size: [3, 5],
                    locked,
                    selected: true,
                }];
                let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
                let _ = frame(vec![]);
                let output = frame(vec![]).0;
                let thumbnail = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                            Some(mesh.calc_bounds())
                        }
                        egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .expect("the original image is represented by a thumbnail");
                let mut buttons = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.size()
                                == egui::Vec2::splat(snip2pin::pin::METRICS.button) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                buttons.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
                assert_eq!(buttons.len(), 2, "lock and close actions must be visible");
                let viewport = Rect::from_min_size(Pos2::ZERO, size);
                for rect in [thumbnail, buttons[0], buttons[1]] {
                    assert!(viewport.contains_rect(rect), "{width} {locale:?}: {rect:?}");
                }
                let original = egui::vec2(3.0, 5.0);
                let preview = egui::Vec2::from(LAUNCHER.pin_thumbnail)
                    - egui::Vec2::splat(LAUNCHER.pin_thumbnail_padding * 2.0);
                let expected = original * (preview / original).min_elem();
                // Image::paint_at rounds both edges to physical pixels. Their
                // independent rounding can change each painted axis by one pixel.
                let pixel = output.pixels_per_point.recip();
                let error = (thumbnail.size() - expected).abs();
                assert!(
                    error.x <= pixel && error.y <= pixel,
                    "thumbnail must preserve proportional fitting within pixel rounding: width={width}, locale={locale:?}, locked={locked}, actual={thumbnail:?}, expected_size={expected:?}, pixels_per_point={}, error={error:?}",
                    output.pixels_per_point
                );
                let click_at = |point| {
                    let _ = frame(vec![Event::PointerMoved(point)]);
                    let event = |pressed| Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    };
                    let _ = frame(vec![event(true)]);
                    frame(vec![event(false)]).1
                };
                assert!(matches!(
                    click_at(thumbnail.center()),
                    Some(Action::SelectPin(17))
                ));
                assert!(
                    matches!(
                        click_at(buttons[0].center()),
                        Some(Action::Pin(super::super::pin::Action::ToggleLock(17)))
                    ),
                    "lock control must remain clickable: width={width}, locale={locale:?}, locked={locked}, button={:?}",
                    buttons[0]
                );
                let closed = click_at(buttons[1].center());
                if locked {
                    assert!(closed.is_none(), "locked list entries cannot close");
                } else {
                    assert!(matches!(
                        closed,
                        Some(Action::Pin(super::super::pin::Action::ClosePin(17)))
                    ));
                }
                let close_all = visible_text(&output, locale.text(Text::ClosePins));
                let closed = click_at(close_all.center());
                if locked {
                    assert!(closed.is_none());
                } else {
                    assert!(matches!(closed, Some(Action::ClosePins)));
                }
            }
        }
    }
}

#[test]
fn populated_small_home_scrolls_to_all_settings_and_keeps_the_footer_visible() {
    for locale in [Locale::English, Locale::Chinese] {
        let ctx = egui::Context::default();
        let size = egui::vec2(280.0, 300.0);
        let texture = ctx.load_texture(
            "generated-scroll-pin",
            egui::ColorImage::filled([3, 5], egui::Color32::LIGHT_BLUE),
            egui::TextureOptions::default(),
        );
        let pins = (0..8)
            .map(|id| PinEntry {
                display_size: None,
                id,
                name: "",
                texture: &texture,
                size: [3, 5],
                locked: false,
                selected: id == 0,
            })
            .collect::<Vec<_>>();
        let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
        let _ = frame(vec![]);
        let mut visible = [false; 4];
        for _ in 0..12 {
            let output = frame(vec![
                Event::PointerMoved(egui::pos2(size.x - 1.0, size.y / 2.0)),
                Event::MouseWheel {
                    source: egui::MouseWheelSource::Unknown,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -60.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                },
            ])
            .0;
            let footer = visible_text(&output, locale.text(Text::Downloads));
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(footer));
            for (index, key) in [Text::Theme, Text::Icon, Text::Language, Text::Startup]
                .into_iter()
                .enumerate()
            {
                visible[index] |= output.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == locale.text(key) => {
                        let bounds = shape.shape.visual_bounding_rect();
                        shape.clip_rect.contains_rect(bounds)
                            && Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds)
                    }
                    _ => false,
                });
            }
        }
        assert_eq!(
            visible, [true; 4],
            "settings must remain reachable below populated list: {locale:?}"
        );
    }
}

#[test]
fn pin_list_defaults_to_three_and_a_half_font_rows_and_resizes_with_real_pointer_input() {
    for large in [false, true] {
        let ctx = egui::Context::default();
        if large {
            ctx.global_style_mut(|style| {
                for font in style.text_styles.values_mut() {
                    font.size *= 2.0;
                }
            });
        }
        let texture = ctx.load_texture(
            "generated-resizable-list",
            egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
            Default::default(),
        );
        let pins = (1..=20)
            .map(|id| PinEntry {
                id,
                display_size: None,
                name: "",
                texture: &texture,
                size: [2, 2],
                locked: false,
                selected: false,
            })
            .collect::<Vec<_>>();
        let size = egui::vec2(
            LAUNCHER.window[0],
            if large { 1000.0 } else { LAUNCHER.window[1] },
        );
        let frame = |size, events, pins: &[PinEntry<'_>]| {
            frame_with_pins(&ctx, size, events, false, Locale::English, pins)
        };
        let _ = frame(size, vec![], &pins[..1]);
        let few = frame(size, vec![], &pins[..1]).0;
        let few_theme = visible_text(&few, "Theme");
        let handle_id = egui::Id::unique("pin-list-resize").with("__resize_corner");
        assert!(
            ctx.read_response(handle_id).is_none(),
            "one pin needs no empty resize region"
        );
        let _ = frame(size, vec![], &pins);
        let output = frame(size, vec![], &pins).0;
        let well = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .unwrap();
        let row_height = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, .. }
                    if (points[0].x - well.left()).abs() <= 1.0 && points[0].y > well.bottom() =>
                {
                    Some((points[0].y - well.center().y) * 2.0)
                }
                _ => None,
            })
            .unwrap();
        let image_clip = |output: &FullOutput| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
                        Some(shape.clip_rect)
                    }
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                        Some(shape.clip_rect)
                    }
                    _ => None,
                })
                .expect("list viewport")
        };
        let initial_clip = image_clip(&output);
        let mut idle_clip = initial_clip;
        for _ in 0..20 {
            idle_clip = image_clip(&frame(size, vec![], &pins).0);
        }
        assert!(
            (idle_clip.height() - initial_clip.height()).abs() <= 1.0,
            "an idle list must keep its height: before={initial_clip:?}, after={idle_clip:?}"
        );
        assert!((initial_clip.height() - row_height * LAUNCHER.pin_list_visible_rows).abs() <= 1.0);
        assert!(visible_text(&output, "Theme").min.y - few_theme.min.y > row_height);
        let handle = ctx
            .read_response(handle_id)
            .expect("built-in resize handle")
            .rect;
        assert!(
            handle.top() >= initial_clip.bottom(),
            "the handle has a separate band below row actions"
        );
        let grown_window = size + egui::vec2(80.0, 200.0);
        for _ in 0..3 {
            let _ = frame(grown_window, vec![], &pins);
        }
        let from = ctx.read_response(handle_id).unwrap().rect.center();
        let to = from + egui::vec2(0.0, row_height * 1.5);
        let pointer = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        for events in [
            vec![Event::PointerMoved(from)],
            vec![pointer(from, true)],
            vec![Event::PointerMoved(to)],
            vec![pointer(to, false)],
        ] {
            assert!(
                frame(grown_window, events, &pins).1.is_none(),
                "resizing must not route a pin action"
            );
        }
        let _ = frame(grown_window, vec![], &pins);
        let expanded = frame(grown_window, vec![], &pins).0;
        assert!(
            image_clip(&expanded).height() > initial_clip.height() + row_height,
            "large={large}, initial={initial_clip:?}, expanded={:?}, from={from:?}, to={to:?}, handle={:?}",
            image_clip(&expanded),
            ctx.read_response(handle_id).map(|response| response.rect)
        );
        let from = ctx.read_response(handle_id).unwrap().rect.center();
        let to = from - egui::vec2(0.0, row_height * 8.0);
        for events in [
            vec![Event::PointerMoved(from)],
            vec![pointer(from, true)],
            vec![Event::PointerMoved(to)],
            vec![pointer(to, false)],
        ] {
            assert!(frame(grown_window, events, &pins).1.is_none());
        }
        let _ = frame(grown_window, vec![], &pins);
        let compact = frame(grown_window, vec![], &pins).0;
        assert!(
            (image_clip(&compact).height() - row_height).abs() <= 1.0,
            "the user can shrink the list to one row"
        );
        let narrow = egui::vec2(210.0, LAUNCHER.window[1]);
        let _ = frame(narrow, vec![], &pins);
        let output = frame(narrow, vec![], &pins).0;
        let close = visible_text(&output, "×").center();
        let _ = frame(narrow, vec![Event::PointerMoved(close)], &pins);
        assert!(frame(narrow, vec![pointer(close, true)], &pins).1.is_none());
        assert!(matches!(
            frame(narrow, vec![pointer(close, false)], &pins).1,
            Some(Action::Pin(super::super::pin::Action::ClosePin(2)))
        ));
    }
}

#[test]
fn pin_zoom_hint_uses_only_observed_display_size_and_unlocked_focus() {
    for locale in [Locale::Chinese, Locale::English] {
        for selected in [false, true] {
            for locked in [false, true] {
                for pixels in [[1, 1], [4096, 4096]] {
                    for (display_size, small) in [
                        (None, false),
                        (Some(egui::vec2(10.0, 400.0)), true),
                        (Some(egui::vec2(400.0, 10.0)), true),
                        (Some(Vec2::splat(snip2pin::pin::METRICS.button)), false),
                        (Some(Vec2::splat(400.0)), false),
                        (Some(egui::vec2(f32::NAN, 10.0)), false),
                        (Some(egui::vec2(0.0, 10.0)), false),
                        (Some(egui::vec2(-1.0, 10.0)), false),
                    ] {
                        let ctx = egui::Context::default();
                        let texture = ctx.load_texture(
                            "generated-zoom-hint",
                            egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                            Default::default(),
                        );
                        let pins = [PinEntry {
                            display_size,
                            id: 17,
                            name: "",
                            texture: &texture,
                            size: pixels,
                            locked,
                            selected,
                        }];
                        let size = Vec2::from(LAUNCHER.window);
                        let _ = frame_with_pins(&ctx, size, vec![], false, locale, &pins);
                        let (output, action) =
                            frame_with_pins(&ctx, size, vec![], false, locale, &pins);
                        assert!(action.is_none());
                        let hint = output.shapes.iter().find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text)
                                if text.galley.text() == locale.text(Text::PinZoomHint) =>
                            {
                                Some(text)
                            }
                            _ => None,
                        });
                        assert_eq!(hint.is_some(), selected && !locked && small);
                        if let Some(hint) = hint {
                            let warning = ctx.global_style().visuals.warn_fg_color;
                            assert!(
                                hint.galley
                                    .job
                                    .sections
                                    .iter()
                                    .all(|section| section.format.color == warning)
                            );
                            assert!(
                                hint.pos.y
                                    >= visible_text(&output, locale.text(Text::ClosePins)).bottom()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn pin_zoom_hint_wraps_and_remains_reachable_in_narrow_homes_with_large_fonts() {
    for width in [210.0, LAUNCHER.window[0]] {
        for locale in [Locale::Chinese, Locale::English] {
            for large in [false, true] {
                let ctx = egui::Context::default();
                if large {
                    ctx.global_style_mut(|style| {
                        for font in style.text_styles.values_mut() {
                            font.size *= 2.0;
                        }
                    });
                }
                let texture = ctx.load_texture(
                    "generated-wrapped-zoom-hint",
                    egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                    Default::default(),
                );
                let pins = (1..=60)
                    .map(|id| PinEntry {
                        display_size: Some(egui::vec2(10.0, 400.0)),
                        id,
                        name: "Reference",
                        texture: &texture,
                        size: [2, 2],
                        locked: false,
                        selected: id == 60,
                    })
                    .collect::<Vec<_>>();
                let size = egui::vec2(width, LAUNCHER.window[1]);
                let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
                let _ = frame(vec![]);
                let mut output = frame(vec![]).0;
                let mut reachable = false;
                for _ in 0..60 {
                    if let Some((text, clip)) =
                        output.shapes.iter().find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text)
                                if text.galley.text() == locale.text(Text::PinZoomHint) =>
                            {
                                Some((text, shape.clip_rect))
                            }
                            _ => None,
                        })
                    {
                        let rect = Rect::from_min_size(text.pos, text.galley.size());
                        if clip.contains_rect(rect)
                            && Rect::from_min_size(Pos2::ZERO, size).contains_rect(rect)
                        {
                            assert!(!text.galley.elided);
                            let unwrapped = ctx.fonts_mut(|fonts| {
                                let mut job = (*text.galley.job).clone();
                                job.wrap.max_width = f32::INFINITY;
                                fonts.layout_job(job)
                            });
                            if unwrapped.size().x > clip.width() {
                                assert!(text.galley.rows.len() > 1);
                            }
                            reachable = true;
                            break;
                        }
                    }
                    let (next, action) = frame(vec![
                        Event::PointerMoved(egui::pos2(size.x - 1.0, size.y / 2.0)),
                        Event::MouseWheel {
                            source: egui::MouseWheelSource::Unknown,
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -24.0),
                            phase: egui::TouchPhase::Move,
                            modifiers: Modifiers::NONE,
                        },
                    ]);
                    assert!(action.is_none());
                    output = next;
                }
                assert!(reachable, "width={width}, locale={locale:?}, large={large}");
            }
        }
    }
}

#[test]
fn lightweight_rows_keep_extreme_previews_padded_and_names_clear_of_controls() {
    for width in [210.0, LAUNCHER.window[0]] {
        for locale in [Locale::Chinese, Locale::English] {
            for scale in [1.0, 2.0] {
                for pixels in [[2, 4096], [4096, 2], [2, 2]] {
                    let ctx = egui::Context::default();
                    ctx.set_pixels_per_point(scale);
                    ctx.input_mut(|input| input.max_texture_side = 4096);
                    let texture = ctx.load_texture(
                        "generated-extreme-preview",
                        egui::ColorImage::filled(pixels, egui::Color32::LIGHT_BLUE),
                        Default::default(),
                    );
                    let size = egui::vec2(width, LAUNCHER.window[1]);
                    let name = match locale {
                        Locale::Chinese => {
                            "接口参考🧭：这是一条很长的名称，不能侵占操作按钮；名称应在空间不足时截断，完整内容仍可查看，复制和关闭操作始终可达"
                        }
                        Locale::English => {
                            "Reference 🧭 — a very long name that must leave both actions reachable"
                        }
                    };
                    let pins = [PinEntry {
                        display_size: None,
                        id: 17,
                        name,
                        texture: &texture,
                        size: pixels,
                        locked: false,
                        selected: false,
                    }];
                    let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
                    let _ = frame(vec![]);
                    let output = frame(vec![]).0;
                    let well = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                            {
                                Some(rect)
                            }
                            _ => None,
                        })
                        .expect("fixed thumbnail well");
                    assert!(well.fill != egui::Color32::TRANSPARENT);
                    assert!(well.stroke.width > 0.0);
                    let image = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
                                Some(rect.rect)
                            }
                            egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                                Some(mesh.calc_bounds())
                            }
                            _ => None,
                        })
                        .expect("contained generated preview");
                    let pixel = output.pixels_per_point.recip();
                    assert!(
                        well.rect
                            .shrink(LAUNCHER.pin_thumbnail_padding)
                            .expand(pixel / 2.0)
                            .contains_rect(image)
                    );
                    assert!(image.width() * output.pixels_per_point >= 1.0 - f32::EPSILON);
                    assert!(image.height() * output.pixels_per_point >= 1.0 - f32::EPSILON);
                    assert!((image.center() - well.rect.center()).abs().max_elem() <= pixel);
                    assert_eq!(
                        texture.size(),
                        pixels,
                        "only the preview may change its short axis"
                    );
                    let (title, title_clip) = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == name => {
                                Some((text, shape.clip_rect))
                            }
                            _ => None,
                        })
                        .expect("truncated full name");
                    let untruncated = ctx.fonts_mut(|fonts| {
                        let mut job = (*title.galley.job).clone();
                        job.wrap.max_width = f32::INFINITY;
                        fonts.layout_job(job)
                    });
                    assert!(
                        untruncated.size().x > title_clip.width(),
                        "fixture must overflow the name slot"
                    );
                    assert!(title.galley.elided);
                    let close = visible_text(&output, "×");
                    let lock_left = close.center().x
                        - snip2pin::pin::METRICS.button * 1.5
                        - snip2pin::pin::METRICS.button_gap;
                    assert!(
                        title_clip.right() <= lock_left,
                        "names cannot cover either action"
                    );
                    let row_line = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::LineSegment { points, .. }
                                if (points[0].x - well.rect.left()).abs() <= pixel
                                    && points[0].y > well.rect.bottom() =>
                            {
                                Some(points)
                            }
                            _ => None,
                        })
                        .expect("light row separator");
                    assert!(
                        row_line[0].y - well.rect.center().y >= LAUNCHER.pin_row_min_height / 2.0
                    );
                    assert!(
                        row_line[1].x - close.center().x
                            >= snip2pin::pin::METRICS.button / 2.0 - pixel
                    );
                    assert!(
                        width - close.center().x - snip2pin::pin::METRICS.button / 2.0
                            >= LAUNCHER.pin_row_padding[0]
                    );
                    let point = Rect::from_min_size(title.pos, title.galley.size())
                        .intersect(title_clip)
                        .center();
                    let _ = frame(vec![Event::PointerMoved(point)]);
                    let mut tooltip_seen = false;
                    for _ in 0..90 {
                        tooltip_seen |= frame(vec![]).0.shapes.iter().any(|shape| matches!(
                            &shape.shape, egui::Shape::Text(text) if text.galley.text() == name && !text.galley.elided
                        ));
                    }
                    assert!(tooltip_seen, "an elided name must remain readable on hover");
                }
            }
        }
    }
}

#[test]
fn lightweight_rows_use_focus_fill_and_distinguish_duplicate_names() {
    for dark in [false, true] {
        for selected in [false, true] {
            for locked in [false, true] {
                let ctx = egui::Context::default();
                ctx.set_visuals(if dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
                let texture = ctx.load_texture(
                    "generated-light-row",
                    egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                    Default::default(),
                );
                let pins = [17, 18].map(|id| PinEntry {
                    display_size: None,
                    id,
                    name: "Reference",
                    texture: &texture,
                    size: [2, 2],
                    locked,
                    selected: selected && id == 17,
                });
                let size = Vec2::from(LAUNCHER.window);
                let _ = frame_with_pins(&ctx, size, vec![], false, Locale::English, &pins);
                let output = frame_with_pins(&ctx, size, vec![], false, Locale::English, &pins).0;
                let row_fills = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.height() >= LAUNCHER.pin_row_min_height
                                && rect.rect.height()
                                    < LAUNCHER.pin_row_min_height
                                        * LAUNCHER.pin_list_visible_rows
                                && rect.fill != egui::Color32::TRANSPARENT =>
                        {
                            Some(rect.fill)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if selected {
                    assert_eq!(row_fills, [ctx.global_style().visuals.selection.bg_fill]);
                } else {
                    assert!(
                        row_fills
                            .iter()
                            .all(|fill| *fill == egui::Color32::TRANSPARENT),
                        "ordinary rows are transparent in either theme: {row_fills:?}"
                    );
                }
                for id in [17, 18] {
                    visible_text(&output, &format!("#{id} · 2 × 2"));
                }
                let point = visible_text(&output, "×").center();
                let frame =
                    |events| frame_with_pins(&ctx, size, events, false, Locale::English, &pins);
                let _ = frame(vec![Event::PointerMoved(point)]);
                let pointer = |pressed| Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                let _ = frame(vec![pointer(true)]);
                let action = frame(vec![pointer(false)]).1;
                if locked {
                    assert!(action.is_none());
                } else {
                    assert!(matches!(
                        action,
                        Some(Action::Pin(super::super::pin::Action::ClosePin(17)))
                    ));
                }
            }
        }
    }
}

#[test]
fn lightweight_rows_disambiguate_default_names_in_both_languages() {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "generated-default-name-collision",
        egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
        Default::default(),
    );
    let size = Vec2::from(LAUNCHER.window);
    for locale in [Locale::Chinese, Locale::English] {
        let name = Text::PinLabel.with(17).render(locale);
        let pins = [
            PinEntry {
                display_size: None,
                id: 17,
                name: "",
                texture: &texture,
                size: [2, 2],
                locked: false,
                selected: false,
            },
            PinEntry {
                display_size: None,
                id: 18,
                name: &name,
                texture: &texture,
                size: [2, 2],
                locked: true,
                selected: false,
            },
        ];
        let _ = frame_with_pins(&ctx, size, vec![], false, locale, &pins);
        let output = frame_with_pins(&ctx, size, vec![], false, locale, &pins).0;
        for id in [17, 18] {
            visible_text(&output, &format!("#{id} · 2 × 2"));
        }
    }
}

#[test]
fn lightweight_narrow_rows_with_large_fonts_keep_duplicate_ids_and_actions_reachable() {
    for width in [210.0, 280.0] {
        for locale in [Locale::Chinese, Locale::English] {
            let ctx = egui::Context::default();
            ctx.global_style_mut(|style| {
                for font in style.text_styles.values_mut() {
                    font.size *= 2.0;
                }
            });
            let texture = ctx.load_texture(
                "generated-large-narrow-row",
                egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                Default::default(),
            );
            let name = if locale == Locale::Chinese {
                "接口参考🧭：用于检查放大文字和窄窗口的重复名称"
            } else {
                "Reference 🧭 — duplicate long names at two hundred percent font size"
            };
            let pins = [17, 18].map(|id| PinEntry {
                display_size: None,
                id,
                name,
                texture: &texture,
                size: [2, 2],
                locked: false,
                selected: false,
            });
            let size = egui::vec2(width, 900.0);
            let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
            let _ = frame(vec![]);
            let output = frame(vec![]).0;
            let title = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == name => {
                        Some((text, shape.clip_rect))
                    }
                    _ => None,
                })
                .expect("a name remains reachable with large fonts");
            assert!(title.1.width() > 0.0);
            let number = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text().starts_with("#17") => {
                        Some((text, shape.clip_rect))
                    }
                    _ => None,
                })
                .expect("a duplicate's full number takes priority over dimensions");
            assert!(!number.0.galley.elided);
            assert!(
                number
                    .1
                    .contains_rect(Rect::from_min_size(number.0.pos, number.0.galley.size())),
                "width={width}, locale={locale:?}, number={:?}, clip={:?}",
                Rect::from_min_size(number.0.pos, number.0.galley.size()),
                number.1
            );
            let close = visible_text(&output, "×").center();
            let lock = close
                - egui::vec2(
                    snip2pin::pin::METRICS.button + snip2pin::pin::METRICS.button_gap,
                    0.0,
                );
            assert!(title.1.right() < lock.x - snip2pin::pin::METRICS.button / 2.0);
            let well = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(well.right() < title.1.left());
            let point = Rect::from_min_size(title.0.pos, title.0.galley.size())
                .intersect(title.1)
                .center();
            let _ = frame(vec![Event::PointerMoved(point)]);
            let mut full_name_seen = false;
            for _ in 0..90 {
                full_name_seen |= frame(vec![]).0.shapes.iter().any(|shape| {
                    matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == name && !text.galley.elided)
                });
            }
            assert!(full_name_seen);
            for (point, expected) in [
                (lock, super::super::pin::Action::ToggleLock(17)),
                (close, super::super::pin::Action::ClosePin(17)),
            ] {
                let _ = frame(vec![Event::PointerMoved(point)]);
                for pressed in [true, false] {
                    let action = frame(vec![Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }])
                    .1;
                    if pressed {
                        assert!(action.is_none());
                    } else {
                        assert!(matches!(action, Some(Action::Pin(actual)) if actual == expected));
                    }
                }
            }
        }
    }
}

#[test]
fn lightweight_inner_list_reaches_the_sixtieth_pin_and_outer_settings() {
    for width in [210.0, LAUNCHER.window[0]] {
        for locale in [Locale::Chinese, Locale::English] {
            let ctx = egui::Context::default();
            let texture = ctx.load_texture(
                "generated-sixty-row-scroll",
                egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                Default::default(),
            );
            let pins = (1..=60)
                .map(|id| PinEntry {
                    display_size: None,
                    id,
                    name: "",
                    texture: &texture,
                    size: [2, 2],
                    locked: false,
                    selected: false,
                })
                .collect::<Vec<_>>();
            let size = egui::vec2(width, LAUNCHER.window[1]);
            let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
            let _ = frame(vec![]);
            let mut output = frame(vec![]).0;
            let point = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                    {
                        Some(rect.rect.center())
                    }
                    _ => None,
                })
                .expect("inner list preview");
            let last_name = Text::PinLabel.with(60).render(locale);
            let mut last_title = None;
            for _ in 0..80 {
                output = frame(vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        source: egui::MouseWheelSource::Unknown,
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -240.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: Modifiers::NONE,
                    },
                ])
                .0;
                last_title = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == last_name => {
                        let rect = Rect::from_min_size(text.pos, text.galley.size());
                        shape.clip_rect.contains_rect(rect).then_some(rect)
                    }
                    _ => None,
                });
                if last_title.is_some() {
                    break;
                }
            }
            assert!(
                last_title.is_some(),
                "the sixtieth pin can be reached inside the bounded list"
            );
            for _ in 0..30 {
                output = frame(vec![]).0;
            }
            let last_title = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == last_name => {
                        Some(Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                })
                .expect("last row after scrolling settles");
            let close = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "×" => {
                        let rect = Rect::from_min_size(text.pos, text.galley.size());
                        shape.clip_rect.contains_rect(rect).then_some(rect.center())
                    }
                    _ => None,
                })
                .max_by(|a, b| a.y.total_cmp(&b.y))
                .unwrap();
            let lock = close
                - egui::vec2(
                    snip2pin::pin::METRICS.button + snip2pin::pin::METRICS.button_gap,
                    0.0,
                );
            for (point, expected) in [
                (last_title.center(), Action::SelectPin(60)),
                (lock, Action::Pin(super::super::pin::Action::ToggleLock(60))),
                (close, Action::Pin(super::super::pin::Action::ClosePin(60))),
            ] {
                let _ = frame(vec![Event::PointerMoved(point)]);
                let pointer = |pressed| Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                assert!(frame(vec![pointer(true)]).1.is_none());
                let action = frame(vec![pointer(false)]).1;
                match (action, expected) {
                    (Some(Action::SelectPin(actual)), Action::SelectPin(expected)) => {
                        assert_eq!(actual, expected)
                    }
                    (Some(Action::Pin(actual)), Action::Pin(expected)) => {
                        assert_eq!(actual, expected)
                    }
                    _ => panic!("the last row's independent action must remain usable"),
                }
            }
            let mut reachable = [false; 4];
            for _ in 0..24 {
                let output = frame(vec![
                    Event::PointerMoved(egui::pos2(size.x - 1.0, size.y / 2.0)),
                    Event::MouseWheel {
                        source: egui::MouseWheelSource::Unknown,
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -60.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: Modifiers::NONE,
                    },
                ])
                .0;
                for (index, key) in [Text::Theme, Text::Icon, Text::Language, Text::Startup]
                    .into_iter()
                    .enumerate()
                {
                    reachable[index] |= output.shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Text(text) if text.galley.text() == locale.text(key)
                            && shape.clip_rect.contains_rect(Rect::from_min_size(text.pos, text.galley.size()))));
                }
            }
            assert_eq!(reachable, [true; 4]);
        }
    }
}

#[test]
fn lightweight_rows_grow_for_large_fonts_and_long_lists_stay_bounded() {
    for large in [false, true] {
        let ctx = egui::Context::default();
        if large {
            ctx.global_style_mut(|style| {
                for font in style.text_styles.values_mut() {
                    font.size *= 2.0;
                }
            });
        }
        let texture = ctx.load_texture(
            "generated-long-list",
            egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
            Default::default(),
        );
        let pins = (0..60)
            .map(|id| PinEntry {
                display_size: None,
                id,
                name: "Reference 🧭",
                texture: &texture,
                size: [2, 2],
                locked: true,
                selected: false,
            })
            .collect::<Vec<_>>();
        let size = egui::vec2(LAUNCHER.window[0], 1000.0);
        let frame = |events, pins: &[PinEntry<'_>]| {
            frame_with_pins(&ctx, size, events, false, Locale::English, pins)
        };
        let _ = frame(vec![], &pins[..1]);
        let one = frame(vec![], &pins[..1]).0;
        let _ = frame(vec![], &pins);
        let many = frame(vec![], &pins).0;
        let theme_one = visible_text(&one, "Theme");
        let theme_many = visible_text(&many, "Theme");
        let well = one
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("thumbnail well");
        let row_height = one
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, .. }
                    if (points[0].x - well.left()).abs() <= 1.0 && points[0].y > well.bottom() =>
                {
                    Some((points[0].y - well.center().y) * 2.0)
                }
                _ => None,
            })
            .expect("row boundary");
        assert!(theme_many.min.y - theme_one.min.y <= row_height * LAUNCHER.pin_list_visible_rows);
        if large {
            assert!(row_height > LAUNCHER.pin_row_min_height);
        } else {
            assert!((row_height - LAUNCHER.pin_row_min_height).abs() <= 1.0);
        }
        let downloads = visible_text(&many, Locale::English.text(Text::Downloads));
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(downloads));
        let close_all = visible_text(&many, Locale::English.text(Text::ClosePins)).center();
        let _ = frame(vec![Event::PointerMoved(close_all)], &pins);
        for pressed in [true, false] {
            assert!(
                frame(
                    vec![Event::PointerButton {
                        pos: close_all,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE
                    }],
                    &pins
                )
                .1
                .is_none()
            );
        }
    }
}

#[test]
fn pin_rows_center_different_aspect_ratios_in_one_thumbnail_column() {
    let ctx = egui::Context::default();
    let size = Vec2::from(LAUNCHER.window);
    let sizes = [[120, 30], [30, 120], [60, 60]];
    let textures = sizes.map(|size| {
        ctx.load_texture(
            format!("generated-column-{size:?}"),
            egui::ColorImage::filled(size, egui::Color32::LIGHT_BLUE),
            Default::default(),
        )
    });
    let names = ["Wide", "Tall", "Square"];
    let pins = std::array::from_fn::<_, 3, _>(|index| PinEntry {
        display_size: None,
        id: index as u64,
        name: names[index],
        texture: &textures[index],
        size: sizes[index],
        locked: false,
        selected: false,
    });
    let mut images = Vec::new();
    let mut titles = Vec::new();
    let mut pixel = 1.0;
    for index in 0..3 {
        let _ = frame_with_pins(
            &ctx,
            size,
            vec![],
            false,
            Locale::English,
            &pins[index..=index],
        );
        let output = frame_with_pins(
            &ctx,
            size,
            vec![],
            false,
            Locale::English,
            &pins[index..=index],
        )
        .0;
        pixel = output.pixels_per_point.recip();
        images.push(
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.fill_texture_id() == textures[index].id() => {
                        Some(rect.rect)
                    }
                    egui::Shape::Mesh(mesh) if mesh.texture_id == textures[index].id() => {
                        Some(mesh.calc_bounds())
                    }
                    _ => None,
                })
                .expect("generated thumbnail"),
        );
        titles.push(
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == names[index] => {
                        Some(Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                })
                .expect("row title"),
        );
    }
    for index in 0..3 {
        assert!((images[index].center().x - images[0].center().x).abs() <= pixel);
        assert!((titles[index].min.x - titles[0].min.x).abs() <= pixel);
        assert!(
            ((images[index].center().y - titles[index].center().y)
                - (images[0].center().y - titles[0].center().y))
                .abs()
                <= pixel,
            "thumbnail and title blocks must share the same vertical alignment"
        );
        let original = Vec2::new(sizes[index][0] as f32, sizes[index][1] as f32);
        let preview =
            Vec2::from(LAUNCHER.pin_thumbnail) - Vec2::splat(LAUNCHER.pin_thumbnail_padding * 2.0);
        let fitted = original * (preview / original).min_elem();
        assert!((images[index].size() - fitted).abs().max_elem() <= pixel);
    }
}

#[test]
fn pin_row_background_thumbnail_and_name_select_without_entering_edit_mode() {
    for width in [210.0, LAUNCHER.window[0]] {
        for locked in [false, true] {
            for target in ["background", "thumbnail", "name", "lock", "close"] {
                let ctx = egui::Context::default();
                let size = egui::vec2(width, LAUNCHER.window[1]);
                let texture = ctx.load_texture(
                    "generated-row-clicks",
                    egui::ColorImage::filled([100, 30], egui::Color32::WHITE),
                    Default::default(),
                );
                let pins = [PinEntry {
                    display_size: None,
                    id: 41,
                    name: "Name",
                    texture: &texture,
                    size: [100, 30],
                    locked,
                    selected: false,
                }];
                let frame =
                    |events| frame_with_pins(&ctx, size, events, false, Locale::English, &pins);
                let _ = frame(vec![]);
                let output = frame(vec![]).0;
                let title = visible_text(&output, "Name");
                let mut buttons = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.size() == Vec2::splat(snip2pin::pin::METRICS.button) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                buttons.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
                assert_eq!(buttons.len(), 2);
                let thumbnail = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
                            Some(rect.rect)
                        }
                        egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                            Some(mesh.calc_bounds())
                        }
                        _ => None,
                    })
                    .expect("generated thumbnail");
                let point = match target {
                    "background" => {
                        egui::pos2((title.max.x + buttons[0].min.x) / 2.0, title.center().y)
                    }
                    "thumbnail" => thumbnail.center(),
                    "name" => title.center(),
                    "lock" => buttons[0].center(),
                    "close" => buttons[1].center(),
                    _ => unreachable!(),
                };
                let _ = frame(vec![Event::PointerMoved(point)]);
                let pointer = |pressed| Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                assert!(frame(vec![pointer(true)]).1.is_none());
                let action = frame(vec![pointer(false)]).1;
                match target {
                    "lock" => assert!(matches!(
                        action,
                        Some(Action::Pin(super::super::pin::Action::ToggleLock(41)))
                    )),
                    "close" if locked => assert!(action.is_none()),
                    "close" => assert!(matches!(
                        action,
                        Some(Action::Pin(super::super::pin::Action::ClosePin(41)))
                    )),
                    _ => assert!(matches!(action, Some(Action::SelectPin(41)))),
                }
                assert!(!ctx.egui_wants_keyboard_input(), "single click: {target}");
                assert!(
                    frame(vec![Event::Text("must not rename".into())])
                        .1
                        .is_none()
                );
                if matches!(target, "background" | "thumbnail") {
                    assert!(frame(vec![pointer(true)]).1.is_none());
                    assert!(matches!(
                        frame(vec![pointer(false)]).1,
                        Some(Action::SelectPin(41))
                    ));
                    assert!(
                        !ctx.egui_wants_keyboard_input(),
                        "only a name double click can enter editing: {target}"
                    );
                }
            }
        }
    }
}

#[test]
fn duplicate_pin_names_double_click_and_edit_by_identity() {
    let ctx = egui::Context::default();
    let size = Vec2::from(LAUNCHER.window);
    let texture = ctx.load_texture(
        "generated-duplicate-names",
        egui::ColorImage::filled([30, 30], egui::Color32::WHITE),
        Default::default(),
    );
    let frame = |events, names: &[String; 2]| {
        let pins = std::array::from_fn::<_, 2, _>(|index| PinEntry {
            display_size: None,
            id: 17 + index as u64,
            name: &names[index],
            texture: &texture,
            size: [30, 30],
            locked: index == 1,
            selected: false,
        });
        frame_with_pins(&ctx, size, events, false, Locale::English, &pins)
    };
    let mut names = ["Same name".to_owned(), "Same name".to_owned()];
    for index in 0..2 {
        let _ = frame(vec![], &names);
        let output = frame(vec![], &names).0;
        let mut titles = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == names[index] => Some(
                    shape
                        .shape
                        .visual_bounding_rect()
                        .intersect(shape.clip_rect),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        titles.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        let point = titles[if index == 0 { 0 } else { titles.len() - 1 }].center();
        let _ = frame(vec![Event::PointerMoved(point)], &names);
        for click in 0..2 {
            for pressed in [true, false] {
                let action = frame(
                    vec![Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }],
                    &names,
                )
                .1;
                if !pressed && click == 0 {
                    assert!(
                        matches!(action, Some(Action::SelectPin(id)) if id == 17 + index as u64)
                    );
                } else {
                    assert!(
                        action.is_none(),
                        "double click must not select again: row={index}, click={click}, pressed={pressed}"
                    );
                }
            }
        }
        let _ = frame(vec![], &names);
        assert!(ctx.egui_wants_keyboard_input());
        let _ = frame(
            vec![Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    command: true,
                    ..Modifiers::NONE
                },
            }],
            &names,
        );
        let replacement = format!("Renamed {}", 17 + index);
        assert!(matches!(
            frame(vec![Event::Text(replacement.clone())], &names).1,
            Some(Action::RenamePin(id, value)) if id == 17 + index as u64 && value == replacement
        ));
        names[index] = replacement;
        let _ = frame(
            vec![Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            &names,
        );
        assert!(!ctx.egui_wants_keyboard_input());
        assert_eq!(
            names[1 - index],
            if index == 0 {
                "Same name"
            } else {
                "Renamed 17"
            }
        );
        // Separate the two editing gestures beyond egui's triple-click window.
        for _ in 0..40 {
            let _ = frame(vec![], &names);
        }
    }
}

#[test]
fn pin_names_double_click_to_edit_and_keep_spaces_and_unicode() {
    for width in [210.0, LAUNCHER.window[0]] {
        for locale in [Locale::English, Locale::Chinese] {
            let ctx = egui::Context::default();
            let size = egui::vec2(width, LAUNCHER.window[1]);
            let texture = ctx.load_texture(
                "generated-name-pin",
                egui::ColorImage::filled([3, 5], egui::Color32::WHITE),
                Default::default(),
            );
            let frame = |events, name: &str| {
                frame_with_pins(
                    &ctx,
                    size,
                    events,
                    false,
                    locale,
                    &[PinEntry {
                        display_size: None,
                        id: 17,
                        name,
                        texture: &texture,
                        size: [3, 5],
                        locked: true,
                        selected: false,
                    }],
                )
            };
            let _ = frame(vec![], "");
            let output = frame(vec![], "").0;
            let point = visible_text(&output, &Text::PinLabel.with(17).render(locale)).center();
            let _ = frame(vec![Event::PointerMoved(point)], "");
            for click in 0..2 {
                for pressed in [true, false] {
                    let action = frame(
                        vec![Event::PointerButton {
                            pos: point,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        }],
                        "",
                    )
                    .1;
                    if pressed || click == 1 {
                        assert!(
                            action.is_none(),
                            "double click must enter editing without locating again"
                        );
                    } else {
                        assert!(matches!(action, Some(Action::SelectPin(17))));
                    }
                }
            }
            let _ = frame(vec![], "");
            let mut name = String::new();
            for text in ["接口", " ", "Reference very long screenshot name"] {
                let action = frame(vec![Event::Text(text.into())], &name).1;
                name.push_str(text);
                assert!(matches!(action, Some(Action::RenamePin(17, value)) if value == name));
            }
            assert!(
                frame(vec![], &name).1.is_none(),
                "idle fields must not emit updates"
            );
            let modifiers = Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            };
            let _ = frame(
                vec![Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                &name,
            );
            let cleared = frame(
                vec![Event::Key {
                    key: egui::Key::Backspace,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                &name,
            )
            .1;
            assert!(matches!(cleared, Some(Action::RenamePin(17, value)) if value.is_empty()));
            let output = frame(vec![], "").0;
            assert!(visible_text(&output, &Text::PinLabel.with(17).render(locale)).is_positive());
        }
    }
}

#[test]
fn removing_a_row_during_a_press_cannot_close_the_pin_that_takes_its_place() {
    let ctx = egui::Context::default();
    let size = Vec2::from(LAUNCHER.window);
    let texture = ctx.load_texture(
        "generated-list-identity",
        egui::ColorImage::filled([3, 5], egui::Color32::WHITE),
        egui::TextureOptions::default(),
    );
    let pins = [17, 18].map(|id| PinEntry {
        display_size: None,
        id,
        name: "",
        texture: &texture,
        size: [3, 5],
        locked: false,
        selected: false,
    });
    let frame = |events, pins: &[PinEntry<'_>]| {
        frame_with_pins(&ctx, size, events, false, Locale::English, pins)
    };
    let _ = frame(vec![], &pins);
    let output = frame(vec![], &pins).0;
    let close = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "×" => {
                Some(shape.shape.visual_bounding_rect())
            }
            _ => None,
        })
        .min_by(|a, b| a.min.y.total_cmp(&b.min.y))
        .expect("first row close button")
        .center();
    let _ = frame(vec![Event::PointerMoved(close)], &pins);
    let pointer = |pressed| Event::PointerButton {
        pos: close,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let _ = frame(vec![pointer(true)], &pins);
    assert!(
        frame(vec![pointer(false)], &pins[1..]).1.is_none(),
        "releasing an old row's press must not close a different pin"
    );
}

#[test]
fn dragged_row_leaves_its_slot_and_follows_the_original_grab_point() {
    for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
        let ctx = egui::Context::default();
        ctx.set_theme(theme);
        let size = egui::vec2(LAUNCHER.window[0], 1000.0);
        let texture = ctx.load_texture(
            "generated floating row",
            egui::ColorImage::filled([8, 8], egui::Color32::WHITE),
            Default::default(),
        );
        let pins = [(3, "Charlie"), (2, "Bravo"), (1, "Alpha")].map(|(id, name)| PinEntry {
            id,
            name,
            texture: &texture,
            size: [8, 8],
            display_size: None,
            locked: id == 3,
            selected: false,
        });
        let frame = |events| frame_with_pins(&ctx, size, events, false, Locale::English, &pins);
        let _ = frame(vec![]);
        let initial = frame(vec![]).0;
        let name = visible_text(&initial, "Charlie");
        let neighbor = visible_text(&initial, "Bravo");
        let well = initial
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("thumbnail");
        let from = egui::pos2(
            well.left() - LAUNCHER.pin_row_padding[0] / 2.0,
            well.center().y,
        );
        let delta = egui::vec2(45.0, 90.0);
        let pointer = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = frame(vec![Event::PointerMoved(from)]);
        let _ = frame(vec![pointer(from, true)]);
        let (floating, action) = frame(vec![Event::PointerMoved(from + delta)]);
        assert!(action.is_none());
        assert!(
            (visible_text(&floating, "Charlie").min - name.min - delta).length() < 0.01,
            "the row must keep the pointer's original grip offset"
        );
        assert_eq!(
            visible_text(&floating, "Bravo"),
            neighbor,
            "the source slot must not collapse"
        );
        assert_eq!(floating.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Charlie")).count(), 1, "paint only one card, not a duplicate in the list");
        assert!(
            !floating
                .shapes
                .iter()
                .any(|shape| shape.clip_rect.is_finite()
                    && matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == well)),
            "the dragged thumbnail must disappear from its list slot"
        );
        let (cancelled, action) = frame(vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        assert!(action.is_none());
        assert_eq!(visible_text(&cancelled, "Charlie"), name);
        let _ = frame(vec![pointer(from + delta, false)]);
        let restored = frame(vec![]).0;
        assert_eq!(visible_text(&restored, "Charlie"), name);
        assert_eq!(pins.map(|pin| pin.id), [3, 2, 1]);
    }
}

#[test]
fn pin_list_grip_reorders_rows_without_selecting_or_changing_model_order() {
    for locale in [Locale::English, Locale::Chinese] {
        let ctx = egui::Context::default();
        let size = egui::vec2(LAUNCHER.window[0], 1000.0);
        let texture = ctx.load_texture(
            "generated-order",
            egui::ColorImage::filled([8, 8], egui::Color32::WHITE),
            Default::default(),
        );
        let pins = [(30, "Charlie"), (20, "Bravo"), (10, "Alpha")].map(|(id, name)| PinEntry {
            id,
            name,
            texture: &texture,
            size: [8, 8],
            display_size: None,
            locked: id == 30,
            selected: false,
        });
        let frame = |events| frame_with_pins(&ctx, size, events, false, locale, &pins);
        let _ = frame(vec![]);
        let initial = frame(vec![]).0;
        let well = initial
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.size() == Vec2::from(LAUNCHER.pin_thumbnail) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("thumbnail");
        let start = egui::pos2(
            well.left() - LAUNCHER.pin_row_padding[0] / 2.0,
            well.center().y,
        );
        // Dropping over another row's close button must sort, never close it.
        let end = egui::pos2(
            visible_text(&initial, "×").center().x,
            visible_text(&initial, "Alpha").bottom() + 10.0,
        );
        let pointer = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        // A handle click, Escape, and release outside the list must not reorder.
        for cancel in 0..3 {
            for events in [vec![Event::PointerMoved(start)], vec![pointer(start, true)]] {
                assert!(frame(events).1.is_none());
            }
            if cancel != 0 {
                assert!(frame(vec![Event::PointerMoved(end)]).1.is_none());
            }
            if cancel == 1 {
                assert!(
                    frame(vec![Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }])
                    .1
                    .is_none()
                );
            }
            let release = if cancel == 2 {
                egui::pos2(size.x + 20.0, end.y)
            } else if cancel == 1 {
                end
            } else {
                start
            };
            assert!(
                frame(vec![Event::PointerMoved(release), pointer(release, false)])
                    .1
                    .is_none()
            );
            let output = frame(vec![]).0;
            assert!(
                visible_text(&output, "Charlie").center().y
                    < visible_text(&output, "Bravo").center().y
            );
            assert!(
                visible_text(&output, "Bravo").center().y
                    < visible_text(&output, "Alpha").center().y
            );
            assert!(!egui::DragAndDrop::has_any_payload(&ctx));
        }
        for events in [
            vec![Event::PointerMoved(start)],
            vec![pointer(start, true)],
            vec![Event::PointerMoved(start + egui::vec2(0.0, 12.0))],
            vec![Event::PointerMoved(end)],
            vec![pointer(end, false)],
        ] {
            assert!(
                frame(events).1.is_none(),
                "reorder must not select, lock, close or rename"
            );
        }
        let reordered = frame(vec![]).0;
        assert!(
            visible_text(&reordered, "Bravo").center().y
                < visible_text(&reordered, "Alpha").center().y
        );
        assert!(
            visible_text(&reordered, "Alpha").center().y
                < visible_text(&reordered, "Charlie").center().y,
            "the locked first row must move after the last row"
        );
        assert_eq!(
            pins.map(|pin| pin.id),
            [30, 20, 10],
            "list order must not mutate the host's pin storage"
        );
    }
}
