use super::*;
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, Rect, Vec2};

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
            assert!(text.galley.text().ends_with("悬停贴图可复制或关闭。"));
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
                let expected =
                    original * (egui::Vec2::from(LAUNCHER.pin_thumbnail) / original).min_elem();
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
                Event::PointerMoved(egui::pos2(size.x - 12.0, size.y / 2.0)),
                Event::MouseWheel {
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
        id: index as u64,
        name: names[index],
        texture: &textures[index],
        size: sizes[index],
        locked: false,
        selected: false,
    });
    let frame = || frame_with_pins(&ctx, size, vec![], false, Locale::English, &pins);
    let _ = frame();
    let output = frame().0;
    let images = textures.each_ref().map(|texture| {
        output
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
            .expect("generated thumbnail")
    });
    let titles = names.map(|name| visible_text(&output, name));
    let pixel = output.pixels_per_point.recip();
    for index in 0..3 {
        assert!((images[index].center().x - images[0].center().x).abs() <= pixel);
        assert!((titles[index].min.x - titles[0].min.x).abs() <= pixel);
        assert!(
            (images[index].center().y - titles[index].center().y).abs() <= pixel,
            "thumbnail must be vertically centered with its row title"
        );
        let original = Vec2::new(sizes[index][0] as f32, sizes[index][1] as f32);
        let fitted = original * (Vec2::from(LAUNCHER.pin_thumbnail) / original).min_elem();
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
