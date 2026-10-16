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
}

pub struct Model<'a> {
    pub locale: Locale,
    pub pin_count: usize,
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

pub fn show(ui: &mut egui::Ui, model: Model<'_>) -> Option<Action> {
    let locale = model.locale;
    let text = |key| locale.text(key);
    ui.painter()
        .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
    footer(ui, locale);
    let mut action = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::new()
            .inner_margin(LAUNCHER.margin)
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
                if model.pin_count > 0
                    && action_row(
                        ui,
                        &locale.pin_count(model.pin_count),
                        text(Text::CloseAll),
                        true,
                    )
                    .clicked()
                {
                    action = Some(Action::ClosePins);
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
