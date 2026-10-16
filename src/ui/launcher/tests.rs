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
                    pin_count: 1,
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
fn compact_home_keeps_capture_shortcut_and_pin_actions_reachable() {
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
    let close = visible_text(&output, "关闭全部").center();
    assert!(matches!(
        click(&ctx, size, close, false).1,
        Some(Action::ClosePins)
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
            Text::CloseAll,
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
