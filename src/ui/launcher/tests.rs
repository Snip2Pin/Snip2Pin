use super::*;
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, Rect, Vec2};

fn frame(
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<Event>,
    busy: bool,
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
            assert!(text.galley.text().ends_with("双击关闭。"));
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
