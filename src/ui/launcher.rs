use eframe::egui::{self, RichText};
use snip2pin::i18n::{Language, Locale, Text};
use snip2pin::settings::{Icon, Preferences, Theme};

use super::style::LAUNCHER;

pub enum Action {
    Capture,
    ClosePins,
    RecordShortcut,
    Appearance(Theme, Icon),
    Language(Language),
    Startup(bool),
    OpenCaptureSettings,
    Restart,
    SelectPin(u64),
    RenamePin(u64, String),
    Pin(super::pin::Action),
}

pub struct PinEntry<'a> {
    pub id: u64,
    pub name: &'a str,
    pub texture: &'a egui::TextureHandle,
    pub size: [usize; 2],
    /// Observed native content size converted to egui points; never source pixels.
    pub display_size: Option<egui::Vec2>,
    pub locked: bool,
    pub selected: bool,
}

pub struct Model<'a> {
    pub locale: Locale,
    pub pins: &'a [PinEntry<'a>],
    pub message: Option<&'a str>,
    pub global_shortcut: bool,
    pub preferences: Preferences,
    pub icon_texture: Option<&'a egui::TextureHandle>,
    pub recording: bool,
    pub idle: bool,
    pub capturing: bool,
    pub demo: bool,
    pub startup: Option<bool>,
    pub permission_required: bool,
}

pub fn pin_is_small(size: egui::Vec2) -> bool {
    size.is_finite() && size.min_elem() > 0.0 && size.min_elem() < snip2pin::pin::METRICS.button
}

pub fn show(ui: &mut egui::Ui, model: Model<'_>) -> Option<Action> {
    let locale = model.locale;
    let text = |key| locale.text(key);
    ui.painter()
        .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
    footer(ui, locale);
    let mut action = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        let margin = if ui.available_width() < LAUNCHER.window[0] {
            LAUNCHER.compact_margin
        } else {
            LAUNCHER.margin
        };
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(margin, LAUNCHER.margin))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = LAUNCHER.item_gap;
                ui.horizontal(|ui| {
                    if let Some(texture) = model.icon_texture {
                        ui.image((texture.id(), egui::Vec2::splat(LAUNCHER.brand_icon)));
                    }
                    ui.label(
                        RichText::new(snip2pin::APP_NAME)
                            .size(LAUNCHER.brand)
                            .strong(),
                    );
                });
                ui.add_space(LAUNCHER.section_gap);
                ui.label(
                    RichText::new(text(Text::Heading))
                        .size(LAUNCHER.heading)
                        .strong(),
                );
                ui.weak(text(Text::Subtitle));
                ui.add_space(LAUNCHER.section_gap);
                let capture = egui::Button::new(
                    RichText::new(if model.capturing {
                        text(Text::Capturing)
                    } else if model.permission_required {
                        text(Text::ContinueCapture)
                    } else {
                        text(Text::Capture)
                    })
                    .color(LAUNCHER.capture_text),
                )
                .fill(LAUNCHER.capture_fill)
                .stroke(egui::Stroke::NONE)
                .corner_radius(LAUNCHER.button_radius);
                if ui
                    .add_enabled_ui((model.idle || model.capturing) && !model.recording, |ui| {
                        ui.add_sized([ui.available_width(), LAUNCHER.capture_height], capture)
                    })
                    .inner
                    .on_hover_ui(|ui| super::tooltip(ui, text(Text::CaptureHelp)))
                    .clicked()
                {
                    action = Some(Action::Capture);
                }
                ui.add_space(LAUNCHER.action_gap);
                let binding = model.preferences.binding.label();
                let shortcut = if model.global_shortcut {
                    binding
                } else {
                    Text::LocalShortcut.with(binding).render(locale)
                };
                if action_row(
                    ui,
                    &shortcut,
                    if model.recording {
                        text(Text::CancelEdit)
                    } else {
                        text(Text::ChangeShortcut)
                    },
                    model.idle,
                )
                .clicked()
                {
                    action = Some(Action::RecordShortcut);
                }
                if model.recording {
                    ui.weak(text(Text::RecordShortcut));
                }
                if !model.pins.is_empty() {
                    ui.add_space(LAUNCHER.action_gap);
                    egui::CollapsingHeader::new(locale.pin_count(model.pins.len()))
                        .id_salt("pin-list")
                        .default_open(true)
                        .show_unindented(ui, |ui| {
                            let row_height = pin_row_height(ui);
                            let default_height = row_height * LAUNCHER.pin_list_visible_rows;
                            let selected =
                                if model.pins.len() as f32 <= LAUNCHER.pin_list_visible_rows {
                                    pin_list_rows(ui, model.pins, locale, default_height)
                                } else {
                                    let handle = ui.visuals().resize_corner_size;
                                    let min_height = row_height + handle;
                                    let max_height = (ui.ctx().content_rect().height()
                                        - f32::from(LAUNCHER.margin) * 2.0)
                                        .max(min_height)
                                        .min(model.pins.len() as f32 * row_height + handle);
                                    let width = ui.available_width();
                                    egui::Resize::default()
                                        .id(egui::Id::new("pin-list-resize"))
                                        .resizable([false, true])
                                        .with_stroke(false)
                                        .default_size(egui::vec2(width, default_height + handle))
                                        .min_size(egui::vec2(width, min_height))
                                        .max_size(egui::vec2(width, max_height))
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing.y = 0.0;
                                            let selected = pin_list_rows(
                                                ui,
                                                model.pins,
                                                locale,
                                                (ui.available_height() - handle).max(row_height),
                                            );
                                            ui.add_space(handle);
                                            selected
                                        })
                                };
                            if selected.is_some() {
                                action = selected;
                            }
                            if ui
                                .add_enabled(
                                    model.pins.iter().any(|pin| !pin.locked),
                                    egui::Link::new(text(Text::ClosePins)),
                                )
                                .clicked()
                            {
                                action = Some(Action::ClosePins);
                            }
                            if model.pins.iter().any(|pin| {
                                pin.selected
                                    && !pin.locked
                                    && pin.display_size.is_some_and(pin_is_small)
                            }) {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(text(Text::PinZoomHint))
                                            .color(ui.visuals().warn_fg_color),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                }
                if model.permission_required {
                    ui.add_space(LAUNCHER.action_gap);
                    ui.label(text(Text::PermissionTitle));
                    ui.weak(text(Text::PermissionHelp));
                    if ui.button(text(Text::OpenSettings)).clicked() {
                        action = Some(Action::OpenCaptureSettings);
                    }
                    ui.weak(text(Text::RestartHelp));
                    if ui
                        .button(text(Text::Restart))
                        .on_hover_ui(|ui| super::tooltip(ui, text(Text::RestartWarning)))
                        .clicked()
                    {
                        action = Some(Action::Restart);
                    }
                }
                ui.add_space(LAUNCHER.section_gap);
                ui.separator();
                let mut theme = model.preferences.theme;
                let mut icon = model.preferences.icon;
                let mut language = model.preferences.language;
                ui.add_enabled_ui(model.idle && !model.recording, |ui| {
                    setting_row(ui, text(Text::Theme), |ui, label| {
                        egui::ComboBox::from_id_salt("theme")
                            .width(LAUNCHER.control_width)
                            .selected_text(theme.label(locale))
                            .show_ui(ui, |ui| {
                                for choice in [Theme::System, Theme::Light, Theme::Dark] {
                                    ui.selectable_value(&mut theme, choice, choice.label(locale));
                                }
                            })
                            .response
                            .labelled_by(label);
                    });
                    setting_row(ui, text(Text::Icon), |ui, label| {
                        egui::ComboBox::from_id_salt("icon")
                            .width(LAUNCHER.control_width)
                            .selected_text(icon.label(locale))
                            .show_ui(ui, |ui| {
                                for choice in [Icon::Auto, Icon::Line, Icon::Metal] {
                                    ui.selectable_value(&mut icon, choice, choice.label(locale));
                                }
                            })
                            .response
                            .labelled_by(label);
                    });
                    setting_row(ui, text(Text::Language), |ui, label| {
                        egui::ComboBox::from_id_salt("language")
                            .width(LAUNCHER.control_width)
                            .selected_text(language.label(locale))
                            .show_ui(ui, |ui| {
                                for choice in
                                    [Language::System, Language::Chinese, Language::English]
                                {
                                    ui.selectable_value(
                                        &mut language,
                                        choice,
                                        choice.label(locale),
                                    );
                                }
                            })
                            .response
                            .labelled_by(label);
                    });
                    if let Some(mut enabled) = model.startup {
                        setting_row(ui, text(Text::Startup), |ui, label| {
                            if ui
                                .checkbox(&mut enabled, "")
                                .labelled_by(label)
                                .on_hover_ui(|ui| super::tooltip(ui, text(Text::StartupHelp)))
                                .changed()
                            {
                                action = Some(Action::Startup(enabled));
                            }
                        });
                    }
                });
                if theme != model.preferences.theme || icon != model.preferences.icon {
                    action = Some(Action::Appearance(theme, icon));
                }
                if language != model.preferences.language {
                    action = Some(Action::Language(language));
                }
                if model.demo {
                    ui.weak(text(Text::Demo));
                }
                if let Some(message) = model.message {
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
            });
    });
    action
}

fn pin_name_id(id: u64) -> egui::Id {
    egui::Id::new(("pin-name", id))
}

fn pin_list_rows(
    ui: &mut egui::Ui,
    pins: &[PinEntry<'_>],
    locale: Locale,
    height: f32,
) -> Option<Action> {
    let mut action = None;
    egui::ScrollArea::vertical()
        .id_salt("pin-list-scroll")
        .max_height(height)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let names = pins
                .iter()
                .map(|pin| {
                    if pin.name.is_empty() {
                        Text::PinLabel.with(pin.id).render(locale)
                    } else {
                        pin.name.to_owned()
                    }
                })
                .collect::<Vec<_>>();
            let mut counts = std::collections::HashMap::new();
            for name in &names {
                *counts.entry(name.as_str()).or_insert(0_usize) += 1;
            }
            for (pin, name) in pins.iter().zip(&names) {
                let duplicate = counts.get(name.as_str()).is_some_and(|&count| count > 1);
                if let Some(selected) = ui
                    .push_id(("pin-row", pin.id), |ui| {
                        pin_row(ui, pin, locale, name, duplicate)
                    })
                    .inner
                {
                    action = Some(selected);
                }
            }
        });
    action
}

fn pin_text_height(ui: &egui::Ui) -> f32 {
    (ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().button_padding.y * 2.0)
        .max(ui.spacing().interact_size.y)
}

fn pin_row_height(ui: &egui::Ui) -> f32 {
    let text_height = pin_text_height(ui)
        + LAUNCHER.pin_metadata_gap
        + ui.text_style_height(&egui::TextStyle::Small);
    LAUNCHER
        .pin_row_min_height
        .max(LAUNCHER.pin_thumbnail[1].max(text_height) + LAUNCHER.pin_row_padding[1] * 2.0)
}

/// Release the closed pin's editor, including its in-memory undo history.
pub fn forget_pin_name(ctx: &egui::Context, id: u64) {
    let editor = pin_name_id(id);
    ctx.data_mut(|data| data.remove::<egui::text_edit::TextEditState>(editor));
    ctx.memory_mut(|memory| memory.surrender_focus(editor));
}

fn pin_row(
    ui: &mut egui::Ui,
    pin: &PinEntry<'_>,
    locale: Locale,
    name: &str,
    duplicate: bool,
) -> Option<Action> {
    let mut action = None;
    let text_height = pin_text_height(ui);
    let metadata_height = ui.text_style_height(&egui::TextStyle::Small);
    let text_block_height = text_height + LAUNCHER.pin_metadata_gap + metadata_height;
    let height = pin_row_height(ui);
    let (row, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            Text::LocatePin.with(pin.id).render(locale),
        )
    });
    if pin.selected || response.contains_pointer() {
        ui.painter().rect_filled(
            row,
            LAUNCHER.button_radius,
            if pin.selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    let content = row.shrink2(egui::Vec2::from(LAUNCHER.pin_row_padding));
    let close_text = egui::WidgetText::from("×").into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let button_size = egui::vec2(
        snip2pin::pin::METRICS
            .button
            .max(close_text.size().x + ui.spacing().button_padding.x * 2.0),
        snip2pin::pin::METRICS
            .button
            .max(text_height + ui.spacing().button_padding.y * 2.0),
    );
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(content.right() - button_size.x / 2.0, row.center().y),
        button_size,
    );
    let lock_rect = close_rect.translate(-egui::vec2(
        button_size.x + snip2pin::pin::METRICS.button_gap,
        0.0,
    ));
    let close = ui
        .add_enabled_ui(!pin.locked, |ui| {
            ui.put(
                close_rect,
                egui::Button::new(egui::RichText::new("×").weak()).frame_when_inactive(false),
            )
        })
        .inner
        .on_hover_text(locale.text(Text::Close));
    close.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled() && !pin.locked,
            locale.text(Text::Close),
        )
    });
    if close.clicked() {
        action = Some(Action::Pin(super::pin::Action::ClosePin(pin.id)));
    }
    let lock = ui
        .put(
            lock_rect,
            egui::Button::new("")
                .selected(pin.locked)
                .frame_when_inactive(false),
        )
        .on_hover_text(locale.text(if pin.locked { Text::Unlock } else { Text::Lock }));
    lock.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            locale.text(if pin.locked { Text::Unlock } else { Text::Lock }),
        )
    });
    super::icons::paint_lock_icon(
        ui,
        lock.rect,
        pin.locked,
        if pin.locked {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        },
    );
    if lock.clicked() {
        action = Some(Action::Pin(super::pin::Action::ToggleLock(pin.id)));
    }
    let thumbnail = egui::Rect::from_center_size(
        egui::pos2(
            content.left() + LAUNCHER.pin_thumbnail[0] / 2.0,
            row.center().y,
        ),
        egui::Vec2::from(LAUNCHER.pin_thumbnail),
    );
    ui.painter().rect(
        thumbnail,
        LAUNCHER.button_radius,
        ui.visuals().faint_bg_color,
        egui::Stroke::new(
            ui.ctx().pixels_per_point().recip(),
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ),
        egui::StrokeKind::Inside,
    );
    let preview = thumbnail.shrink(LAUNCHER.pin_thumbnail_padding);
    let size = egui::vec2(pin.size[0] as f32, pin.size[1] as f32);
    let fitted = (size * (preview.size() / size).min_elem())
        .max(egui::Vec2::splat(ui.ctx().pixels_per_point().recip()))
        .min(preview.size());
    egui::Image::new((pin.texture.id(), fitted))
        .paint_at(ui, egui::Rect::from_center_size(preview.center(), fitted));
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(
            thumbnail.right() + LAUNCHER.pin_column_gap,
            row.center().y - text_block_height / 2.0,
        ),
        egui::pos2(
            (lock.rect.left() - LAUNCHER.pin_column_gap)
                .max(thumbnail.right() + LAUNCHER.pin_column_gap),
            row.center().y + text_block_height / 2.0,
        ),
    );
    let editor = pin_name_id(pin.id);
    ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
        let text_clip = egui::Rect::from_min_max(
            egui::pos2(text_rect.left(), row.top()),
            egui::pos2(text_rect.right(), row.bottom()),
        );
        ui.set_clip_rect(ui.clip_rect().intersect(text_clip));
        let name_rect =
            egui::Rect::from_min_size(text_rect.min, egui::vec2(text_rect.width(), text_height));
        if ui.memory(|memory| memory.has_focus(editor)) {
            let mut name = pin.name.to_owned();
            let field = ui.put(
                name_rect,
                egui::TextEdit::singleline(&mut name)
                    .id(editor)
                    .hint_text(Text::PinLabel.with(pin.id).render(locale)),
            );
            if field.changed() {
                action = Some(Action::RenamePin(pin.id, name));
            }
        } else {
            let label = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(name_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.add(
                            egui::Label::new(name)
                                .truncate()
                                .sense(egui::Sense::click()),
                        )
                    },
                )
                .inner;
            if label.double_clicked() {
                ui.memory_mut(|memory| memory.request_focus(editor));
                ui.ctx().request_repaint();
            } else if label.clicked() {
                action = Some(Action::SelectPin(pin.id));
            }
        }
        let dimensions = format!("{} × {}", pin.size[0], pin.size[1]);
        let metadata = if duplicate {
            format!("#{} · {dimensions}", pin.id)
        } else {
            dimensions
        };
        let metadata_rect = egui::Rect::from_min_size(
            egui::pos2(
                text_rect.left(),
                name_rect.bottom() + LAUNCHER.pin_metadata_gap,
            ),
            egui::vec2(text_rect.width(), metadata_height),
        );
        let full_metadata = egui::WidgetText::from(egui::RichText::new(&metadata).small())
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Small,
            );
        let shown_metadata = if duplicate && full_metadata.size().x > metadata_rect.width() {
            format!("#{}", pin.id)
        } else {
            metadata.clone()
        };
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(metadata_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(shown_metadata).small().weak())
                        .truncate()
                        .show_tooltip_when_elided(false),
                )
                .on_hover_text(metadata);
            },
        );
    });
    if action.is_none()
        && response.clicked()
        && response
            .interact_pointer_pos()
            .is_some_and(|point| !lock.rect.contains(point) && !close.rect.contains(point))
    {
        action = Some(Action::SelectPin(pin.id));
    }
    ui.painter().line_segment(
        [
            egui::pos2(content.left(), row.bottom()),
            egui::pos2(content.right(), row.bottom()),
        ],
        egui::Stroke::new(
            ui.ctx().pixels_per_point().recip(),
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ),
    );
    action
}

fn footer(ui: &mut egui::Ui, locale: Locale) {
    egui::Panel::bottom("launcher-footer")
        .resizable(false)
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(
            LAUNCHER.margin,
            LAUNCHER.footer_margin,
        )))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.weak(concat!("v", env!("CARGO_PKG_VERSION")));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.hyperlink_to(
                        locale.text(Text::Downloads),
                        concat!(env!("CARGO_PKG_REPOSITORY"), "/releases"),
                    )
                    .on_hover_ui(|ui| super::tooltip(ui, locale.text(Text::DownloadsHelp)));
                });
            });
        });
}

fn setting_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    control: impl FnOnce(&mut egui::Ui, egui::Id) -> R,
) -> R {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), LAUNCHER.row_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let label = ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                control(ui, label.id)
            })
            .inner
        },
    )
    .inner
}

fn action_row(ui: &mut egui::Ui, label: &str, action: &str, enabled: bool) -> egui::Response {
    let link_width = egui::WidgetText::from(action)
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            ui.available_width(),
            egui::TextStyle::Body,
        )
        .size()
        .x;
    ui.horizontal_wrapped(|ui| {
        ui.weak(label);
        if ui.available_size_before_wrap().x < link_width {
            ui.end_row();
        }
        ui.add_space((ui.available_size_before_wrap().x - link_width).max(0.0));
        ui.add_enabled(enabled, egui::Link::new(action))
    })
    .inner
}

#[cfg(test)]
mod tests;
