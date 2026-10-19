use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use snip2pin::geometry::{self, PixelRect};

use super::style::OVERLAY;

pub struct Selection {
    pub texture: TextureHandle,
    pub pixels: [u32; 2],
    /// Source pixels per native window coordinate step, independent of UI zoom.
    pub pixel_step: [u32; 2],
    /// Front-to-back window regions in this frozen display's source pixels.
    pub windows: Vec<PixelRect>,
    pub press: Option<Pos2>,
    pub start: Option<Pos2>,
    pub focus_requested: bool,
}

pub enum Action {
    Activate,
    Cancel,
    Select(PixelRect),
}

fn image_rect(bounds: Rect, pixels: [u32; 2], rect: PixelRect) -> Rect {
    let units = bounds.size() / egui::vec2(pixels[0] as f32, pixels[1] as f32);
    Rect::from_min_size(
        bounds.min + egui::vec2(rect.x as f32, rect.y as f32) * units,
        egui::vec2(rect.width as f32, rect.height as f32) * units,
    )
}

impl Selection {
    fn window_at(&self, bounds: Rect, point: Pos2) -> Option<PixelRect> {
        bounds.contains(point).then_some(())?;
        self.windows.iter().copied().find(|window| {
            let rect = image_rect(bounds, self.pixels, *window);
            point.x >= rect.min.x
                && point.x < rect.max.x
                && point.y >= rect.min.y
                && point.y < rect.max.y
        })
    }
}

pub fn show(ui: &mut egui::Ui, selection: &mut Selection, active: bool) -> Option<Action> {
    let bounds = ui.max_rect();
    let activate = !active
        && ui.input(|input| {
            input.events.iter().any(|event| match event {
                egui::Event::PointerMoved(point)
                | egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => bounds.contains(*point),
                _ => false,
            })
        });
    if active && !selection.focus_requested {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        selection.focus_requested = true;
    }
    if !active {
        selection.focus_requested = false;
        selection.press = None;
    }
    ui.painter().image(
        selection.texture.id(),
        bounds,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    let response = ui.allocate_rect(bounds, Sense::click_and_drag());
    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    if ui.input(|input| input.key_pressed(egui::Key::Escape) || input.viewport().close_requested())
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        return Some(Action::Cancel);
    }
    if (active || activate)
        && let Some(point) = ui.input(|input| {
            input.events.iter().rev().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            })
        })
    {
        selection.press = bounds.contains(point).then_some(point);
    }
    // A complete fast gesture may arrive before the next frame, leaving no
    // in-progress drag response. Use egui's own click classification, retaining
    // the original press instead of guessing a new threshold or anchor.
    let blank_press = selection
        .press
        .is_some_and(|point| selection.window_at(bounds, point).is_none());
    let released_drag = selection.press.is_some()
        && ui.input(|input| {
            input.pointer.primary_released() && (!input.pointer.primary_clicked() || blank_press)
        });
    if response.drag_started_by(egui::PointerButton::Primary)
        || released_drag
        || (blank_press && ui.input(|input| input.pointer.primary_down()))
    {
        selection.start = selection
            .press
            .or_else(|| ui.input(|input| input.pointer.press_origin()));
    }
    let current = ui.input(|input| input.pointer.interact_pos());
    if let (Some(start), Some(current)) = (selection.start, current) {
        let selected = geometry::selection_with_step(
            [
                f64::from(start.x - bounds.min.x),
                f64::from(start.y - bounds.min.y),
            ],
            [
                f64::from(current.x - bounds.min.x),
                f64::from(current.y - bounds.min.y),
            ],
            [f64::from(bounds.width()), f64::from(bounds.height())],
            selection.pixels,
            selection.pixel_step,
        );
        if let Some(pixels) = selected {
            ui.painter().rect_stroke(
                image_rect(bounds, selection.pixels, pixels),
                0,
                Stroke::new(OVERLAY.stroke, Color32::WHITE),
                StrokeKind::Inside,
            );
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) || released_drag {
            if selected.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            }
            selection.start = None;
            selection.press = None;
            return selected.map(Action::Select);
        }
    }
    let hovered = (active || activate)
        .then(|| current.and_then(|point| selection.window_at(bounds, point)))
        .flatten();
    if selection.start.is_none()
        && let Some(rect) = hovered
    {
        ui.painter().rect_stroke(
            image_rect(bounds, selection.pixels, rect),
            0,
            Stroke::new(OVERLAY.stroke, OVERLAY.highlight),
            StrokeKind::Inside,
        );
        // The layer can receive a complete click on its very first frame,
        // before egui has a prior-frame widget hit map. The retained in-bounds
        // press binds this layer; egui still decides whether the gesture clicked.
        if ui.input(|input| input.pointer.primary_clicked())
            && selection
                .press
                .and_then(|point| selection.window_at(bounds, point))
                == Some(rect)
        {
            selection.press = None;
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            return Some(Action::Select(rect));
        }
    }
    if ui.input(|input| input.pointer.primary_released()) {
        selection.press = None;
    }
    if active && selection.start.is_none() {
        let padding = egui::Vec2::from(OVERLAY.hint_padding);
        let width = OVERLAY.hint_size[0].min(bounds.width());
        let galley = ui.painter().layout(
            crate::i18n::locale(ui.ctx())
                .text(if selection.windows.is_empty() {
                    crate::i18n::Text::SelectionHelp
                } else {
                    crate::i18n::Text::WindowSelectionHelp
                })
                .into(),
            egui::FontId::proportional(OVERLAY.hint_font),
            Color32::WHITE,
            (width - padding.x * 2.0).max(1.0),
        );
        let hint_size = egui::vec2(
            width,
            (galley.size().y + padding.y * 2.0).max(OVERLAY.hint_size[1]),
        );
        let hint = Rect::from_min_size(
            Pos2::new(
                bounds.center().x - width / 2.0,
                bounds.min.y + OVERLAY.hint_top,
            ),
            hint_size,
        );
        ui.painter()
            .rect_filled(hint, OVERLAY.hint_radius, OVERLAY.hint_fill);
        ui.painter()
            .galley(hint.center() - galley.size() / 2.0, galley, Color32::WHITE);
    }
    activate.then_some(Action::Activate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hovering_identifies_the_front_window_and_a_click_selects_the_exact_preview() {
        let ctx = egui::Context::default();
        let front = PixelRect {
            x: 40,
            y: 50,
            width: 120,
            height: 90,
        };
        let mut selection = Selection {
            texture: ctx.load_texture(
                "generated windows",
                egui::ColorImage::filled([2, 2], Color32::BLACK),
                Default::default(),
            ),
            pixels: [640, 480],
            pixel_step: [1, 1],
            windows: vec![
                front,
                PixelRect {
                    x: 20,
                    y: 20,
                    width: 300,
                    height: 200,
                },
            ],
            press: None,
            start: None,
            focus_requested: true,
        };
        let point = egui::pos2(80.0, 80.0);
        for (frame, events) in [
            vec![egui::Event::PointerMoved(point)],
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ]
        .into_iter()
        .enumerate()
        {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let result = show(ui, &mut selection, true);
                    if frame == 2 {
                        assert!(
                            matches!(result, Some(Action::Select(rect)) if rect == front),
                            "click must select the highlighted front window"
                        );
                    } else {
                        assert!(result.is_none());
                    }
                },
            );
            output.textures_delta.clear();
            if frame == 0 {
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == Rect::from_min_size(egui::pos2(40.0, 50.0), egui::vec2(120.0, 90.0)))), "hover must outline the selected window");
                assert_eq!(output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == OVERLAY.highlight)).count(), 1, "only the recognized window may be highlighted");
            }
        }
    }

    #[test]
    fn dragging_from_a_recognized_window_uses_the_drag_rectangle_even_when_events_are_batched() {
        for batched in [false, true] {
            let ctx = egui::Context::default();
            let mut selection = Selection {
                texture: ctx.load_texture(
                    "generated drag",
                    egui::ColorImage::filled([2, 2], Color32::BLACK),
                    Default::default(),
                ),
                pixels: [640, 480],
                pixel_step: [1, 1],
                windows: vec![PixelRect {
                    x: 40,
                    y: 50,
                    width: 120,
                    height: 90,
                }],
                press: None,
                start: None,
                focus_requested: true,
            };
            let start = egui::pos2(80.0, 80.0);
            let end = egui::pos2(200.0, 200.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let frames = if batched {
                vec![
                    vec![egui::Event::PointerMoved(start)],
                    vec![
                        button(start, true),
                        egui::Event::PointerMoved(end),
                        button(end, false),
                    ],
                ]
            } else {
                vec![
                    vec![egui::Event::PointerMoved(start)],
                    vec![button(start, true)],
                    vec![egui::Event::PointerMoved(end)],
                    vec![button(end, false)],
                ]
            };
            let last = frames.len() - 1;
            for (index, events) in frames.into_iter().enumerate() {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(640.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let result = show(ui, &mut selection, true);
                        if index == last {
                            assert!(
                                matches!(
                                    result,
                                    Some(Action::Select(PixelRect {
                                        x: 80,
                                        y: 80,
                                        width: 120,
                                        height: 120
                                    }))
                                ),
                                "drag must override window detection, batched={batched}"
                            );
                        }
                    },
                );
                output.textures_delta.clear();
                if index == last {
                    let outlines: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.stroke.color != Color32::TRANSPARENT =>
                            {
                                Some(rect.stroke.color)
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(
                        outlines,
                        vec![Color32::WHITE],
                        "dragging must show only the region outline"
                    );
                }
            }
        }
    }

    #[test]
    fn window_clicks_reject_changed_targets_secondary_buttons_and_unpaired_releases() {
        for (press, release, button, select) in [
            (
                Some(egui::pos2(80.0, 80.0)),
                egui::pos2(80.0, 80.0),
                egui::PointerButton::Primary,
                true,
            ),
            (
                Some(egui::pos2(39.0, 80.0)),
                egui::pos2(40.0, 80.0),
                egui::PointerButton::Primary,
                false,
            ),
            (
                Some(egui::pos2(40.0, 80.0)),
                egui::pos2(39.0, 80.0),
                egui::PointerButton::Primary,
                false,
            ),
            (
                Some(egui::pos2(80.0, 80.0)),
                egui::pos2(80.0, 80.0),
                egui::PointerButton::Secondary,
                false,
            ),
            (
                Some(egui::pos2(400.0, 300.0)),
                egui::pos2(400.0, 300.0),
                egui::PointerButton::Primary,
                false,
            ),
            (
                None,
                egui::pos2(80.0, 80.0),
                egui::PointerButton::Primary,
                false,
            ),
        ] {
            let ctx = egui::Context::default();
            let mut selection = Selection {
                texture: ctx.load_texture(
                    "generated click guards",
                    egui::ColorImage::filled([2, 2], Color32::BLACK),
                    Default::default(),
                ),
                pixels: [640, 480],
                pixel_step: [1, 1],
                windows: vec![
                    PixelRect {
                        x: 40,
                        y: 50,
                        width: 120,
                        height: 90,
                    },
                    PixelRect {
                        x: 20,
                        y: 20,
                        width: 300,
                        height: 200,
                    },
                ],
                press: None,
                start: None,
                focus_requested: true,
            };
            let mut events = vec![egui::Event::PointerMoved(press.unwrap_or(release))];
            if let Some(pos) = press {
                events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            events.push(egui::Event::PointerMoved(release));
            events.push(egui::Event::PointerButton {
                pos: release,
                button,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let result = show(ui, &mut selection, true);
                    assert_eq!(
                        matches!(result, Some(Action::Select(_))),
                        select,
                        "press={press:?}, release={release:?}, button={button:?}"
                    );
                },
            );
            output.textures_delta.clear();
            assert!(
                selection.press.is_none(),
                "release must clear the original press"
            );
            if select {
                assert_eq!(
                    output.platform_output.cursor_icon,
                    egui::CursorIcon::Default
                );
            }
        }
    }

    #[test]
    fn window_clicks_work_on_entering_another_display_at_every_ui_zoom() {
        for zoom in [1.0, 1.25, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(zoom);
            let bounds =
                Rect::from_min_size(egui::pos2(13.0, 17.0), egui::vec2(320.0, 240.0) / zoom);
            let rect = PixelRect {
                x: 40,
                y: 50,
                width: 120,
                height: 90,
            };
            let mut selection = Selection {
                texture: ctx.load_texture(
                    "generated secondary screen",
                    egui::ColorImage::filled([2, 2], Color32::BLACK),
                    Default::default(),
                ),
                pixels: [640, 480],
                pixel_step: [2, 2],
                windows: vec![rect],
                press: None,
                start: None,
                focus_requested: false,
            };
            let point = bounds.min + egui::vec2(40.0, 40.0) / zoom;
            let events = vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_max(Pos2::ZERO, bounds.max)), events, ..Default::default() }, |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                    assert!(matches!(show(ui, &mut selection, false), Some(Action::Select(selected)) if selected == rect));
                });
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn window_click_tolerance_allows_jitter_and_long_hold_tiny_selection() {
        for long_hold in [false, true] {
            let ctx = egui::Context::default();
            let duration = ctx.options(|options| options.input_options.max_click_duration);
            let window = PixelRect {
                x: 40,
                y: 50,
                width: 120,
                height: 90,
            };
            let mut selection = Selection {
                texture: ctx.load_texture(
                    "generated window gesture",
                    egui::ColorImage::filled([2, 2], Color32::BLACK),
                    Default::default(),
                ),
                pixels: [640, 480],
                pixel_step: [1, 1],
                windows: vec![window],
                press: None,
                start: None,
                focus_requested: true,
            };
            let start = egui::pos2(80.0, 80.0);
            let end = start + egui::Vec2::splat(2.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let release_time = if long_hold {
                duration * 2.0
            } else {
                duration / 2.0
            };
            for (index, (time, events)) in [
                (0.0, vec![egui::Event::PointerMoved(start)]),
                (0.01, vec![button(start, true)]),
                (
                    0.01 + release_time,
                    vec![egui::Event::PointerMoved(end), button(end, false)],
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        time: Some(time),
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(640.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let result = show(ui, &mut selection, true);
                        if index == 2 {
                            let expected = if long_hold {
                                PixelRect {
                                    x: 80,
                                    y: 80,
                                    width: 2,
                                    height: 2,
                                }
                            } else {
                                window
                            };
                            assert!(
                                matches!(result, Some(Action::Select(rect)) if rect == expected),
                                "long_hold={long_hold}"
                            );
                        } else {
                            assert!(result.is_none());
                        }
                    },
                );
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn blank_space_allows_direct_tiny_drags_without_first_exceeding_click_tolerance() {
        let ctx = egui::Context::default();
        let mut selection = Selection {
            texture: ctx.load_texture(
                "generated blank region",
                egui::ColorImage::filled([2, 2], Color32::BLACK),
                Default::default(),
            ),
            pixels: [640, 480],
            pixel_step: [1, 1],
            windows: Vec::new(),
            press: None,
            start: None,
            focus_requested: true,
        };
        for (index, events) in [
            vec![
                egui::Event::PointerMoved(egui::pos2(20.0, 20.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(20.0, 20.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![
                egui::Event::PointerMoved(egui::pos2(22.0, 22.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(22.0, 22.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ]
        .into_iter()
        .enumerate()
        {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let action = show(ui, &mut selection, true);
                    if index == 1 {
                        assert!(matches!(
                            action,
                            Some(Action::Select(PixelRect {
                                x: 20,
                                y: 20,
                                width: 2,
                                height: 2
                            }))
                        ));
                    }
                },
            );
            output.textures_delta.clear();
        }
    }

    #[test]
    fn nonempty_tiny_selections_are_allowed_at_each_dpi_and_ui_zoom() {
        for scale in [1.0_f32, 1.5, 2.0, 3.0] {
            for zoom in [1.0, 1.25, 2.0] {
                for (size, valid) in [
                    (egui::vec2(1.0 / scale, 20.0), true),
                    (egui::vec2(20.0, 1.0 / scale), true),
                    (egui::Vec2::splat(2.0), true),
                ] {
                    let ctx = egui::Context::default();
                    ctx.set_zoom_factor(zoom);
                    let bounds = Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(100.0 / zoom));
                    let mut selection = Selection {
                        texture: ctx.load_texture(
                            "generated",
                            egui::ColorImage::filled([2, 2], Color32::WHITE),
                            Default::default(),
                        ),
                        pixels: [(100.0 * scale) as u32; 2],
                        pixel_step: [1, 1],
                        windows: Vec::new(),
                        press: None,
                        start: None,
                        focus_requested: true,
                    };
                    let start = egui::pos2(20.0, 20.0) / zoom;
                    let end = start + size / zoom;
                    let button = |point, pressed| egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    };
                    // First pass the drag threshold, then return to the tiny selection.
                    let frames = [
                        vec![egui::Event::PointerMoved(start)],
                        vec![button(start, true)],
                        vec![egui::Event::PointerMoved(start + egui::Vec2::splat(30.0))],
                        vec![egui::Event::PointerMoved(end)],
                        vec![button(end, false)],
                    ];
                    for (index, events) in frames.into_iter().enumerate() {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(bounds),
                                events,
                                ..Default::default()
                            },
                            |ui| {
                                let action = show(ui, &mut selection, true);
                                if index == 4 {
                                    assert_eq!(
                                        matches!(action, Some(Action::Select(_))),
                                        valid,
                                        "scale={scale} zoom={zoom} size={size:?}"
                                    );
                                }
                            },
                        );
                        output.textures_delta.clear();
                        if index >= 3 {
                            assert_eq!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == Color32::WHITE)), valid);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn translated_selection_hint_wraps_inside_narrow_screens() {
        for locale in [crate::i18n::Locale::English, crate::i18n::Locale::Chinese] {
            for width in [180.0, 340.0, 1000.0] {
                let ctx = egui::Context::default();
                crate::i18n::set_locale(&ctx, locale);
                let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 600.0));
                let mut selection = Selection {
                    texture: ctx.load_texture(
                        "generated",
                        egui::ColorImage::filled([2, 2], Color32::BLACK),
                        Default::default(),
                    ),
                    pixels: [2, 2],
                    pixel_step: [1, 1],
                    windows: Vec::new(),
                    press: None,
                    start: None,
                    focus_requested: true,
                };
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        ..Default::default()
                    },
                    |ui| {
                        show(ui, &mut selection, true);
                    },
                );
                output.textures_delta.clear();
                let shape = output.shapes.iter().find(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == locale.text(crate::i18n::Text::SelectionHelp))).expect("complete translated hint");
                assert!(
                    bounds
                        .expand(1.0)
                        .contains_rect(shape.shape.visual_bounding_rect())
                );
                assert!(
                    shape
                        .clip_rect
                        .expand(1.0)
                        .contains_rect(shape.shape.visual_bounding_rect())
                );
            }
        }
    }

    #[test]
    fn preview_outline_and_released_crop_share_the_native_pixel_grid() {
        let pixels = [2000, 1600];
        for (start, end, expected) in [
            (
                [100.5, 150.5],
                [400.5, 350.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [400.5, 350.5],
                [100.5, 150.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [400.5, 150.5],
                [100.5, 350.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [100.5, 350.5],
                [400.5, 150.5],
                PixelRect {
                    x: 200,
                    y: 300,
                    width: 602,
                    height: 402,
                },
            ),
            (
                [500.5, 600.5],
                [1100.0, 900.0],
                PixelRect {
                    x: 1000,
                    y: 1200,
                    width: 1000,
                    height: 400,
                },
            ),
            (
                [100.5, 150.5],
                [-20.0, -30.0],
                PixelRect {
                    x: 0,
                    y: 0,
                    width: 202,
                    height: 302,
                },
            ),
        ] {
            for zoom in [1.0, 1.25] {
                let ctx = egui::Context::default();
                let bounds =
                    Rect::from_min_size(egui::pos2(13.0, 17.0), egui::vec2(1000.0, 800.0) / zoom);
                let start = bounds.min + egui::Vec2::from(start) / zoom;
                let end = bounds.min + egui::Vec2::from(end) / zoom;
                let mut selection = Selection {
                    texture: ctx.load_texture(
                        "generated selection",
                        egui::ColorImage::filled([2, 2], Color32::BLACK),
                        egui::TextureOptions::NEAREST,
                    ),
                    pixels,
                    pixel_step: [2, 2],
                    windows: Vec::new(),
                    press: None,
                    start: None,
                    focus_requested: true,
                };
                let button = |pos, pressed| egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                let frames = [
                    vec![egui::Event::PointerMoved(start)],
                    vec![button(start, true)],
                    vec![egui::Event::PointerMoved(end)],
                    vec![button(end, false)],
                ];
                let expected_outline = Rect::from_min_size(
                    bounds.min + egui::vec2(expected.x as f32, expected.y as f32) / (2.0 * zoom),
                    egui::vec2(expected.width as f32, expected.height as f32) / (2.0 * zoom),
                );
                for (frame, events) in frames.into_iter().enumerate() {
                    let mut output = ctx.run_ui(egui::RawInput {
                        screen_rect: Some(Rect::from_min_max(Pos2::ZERO, bounds.max)),
                        events,
                        ..Default::default()
                    }, |ui| {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            let action = show(ui, &mut selection, true);
                            if frame == 3 {
                                assert!(matches!(action, Some(Action::Select(rect)) if rect == expected), "released crop must match source grid, zoom={zoom}");
                            }
                        });
                    });
                    output.textures_delta.clear();
                    if frame >= 2 {
                        let outline = output
                            .shapes
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Rect(rect) if rect.stroke.color == Color32::WHITE => {
                                    Some(rect.rect)
                                }
                                _ => None,
                            })
                            .expect("selection outline");
                        assert!(outline.min.distance(expected_outline.min) < 0.001);
                        assert!(outline.max.distance(expected_outline.max) < 0.001);
                    }
                }
            }
        }
    }

    #[test]
    fn completing_or_cancelling_selection_restores_the_cursor() {
        for cancel in [false, true] {
            let ctx = egui::Context::default();
            let texture = ctx.load_texture(
                "cursor",
                egui::ColorImage::filled([2, 2], Color32::WHITE),
                egui::TextureOptions::NEAREST,
            );
            let mut selection = Selection {
                texture,
                pixels: [640, 480],
                pixel_step: [1, 1],
                windows: Vec::new(),
                press: None,
                start: None,
                focus_requested: false,
            };
            let start = egui::pos2(40.0, 60.0);
            let end = egui::pos2(220.0, 180.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let last_event = if cancel {
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }
            } else {
                button(end, false)
            };
            let frames = [
                vec![egui::Event::PointerMoved(start)],
                vec![button(start, true)],
                vec![egui::Event::PointerMoved(end)],
                vec![last_event],
            ];
            for (index, events) in frames.into_iter().enumerate() {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(640.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let action = show(ui, &mut selection, true);
                        if index == 3 {
                            assert!(matches!(
                                (&action, cancel),
                                (Some(Action::Cancel), true) | (Some(Action::Select(_)), false)
                            ));
                        }
                    },
                );
                output.textures_delta.clear();
                assert_eq!(
                    output.platform_output.cursor_icon,
                    if index == 3 {
                        egui::CursorIcon::Default
                    } else {
                        egui::CursorIcon::Crosshair
                    }
                );
            }
        }
    }

    #[test]
    fn displays_keep_the_same_appearance_without_screen_highlighting() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "selection",
            egui::ColorImage::filled([2, 2], Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let mut selection = Selection {
            texture,
            pixels: [2, 2],
            pixel_step: [1, 1],
            windows: Vec::new(),
            press: None,
            start: None,
            focus_requested: false,
        };
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0));
        for active in [true, false] {
            let mut frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    ..Default::default()
                },
                |ui| {
                    show(ui, &mut selection, active);
                },
            );
            frame.textures_delta.clear();
            assert!(
                !frame.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == bounds)
                }),
                "display activation must not add a screen border or dimming layer"
            );
            assert_eq!(
                frame.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .any(|command| { matches!(command, egui::ViewportCommand::Focus) }),
                active,
                "inactive displays must not steal keyboard focus"
            );
        }
        let mut frame = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                events: vec![egui::Event::PointerMoved(egui::pos2(50.0, 50.0))],
                ..Default::default()
            },
            |ui| {
                assert!(matches!(
                    show(ui, &mut selection, false),
                    Some(Action::Activate)
                ));
            },
        );
        frame.textures_delta.clear();
        assert!(frame.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == selection.texture.id())
        }), "switching displays must not clear the frozen desktop");
    }
}
