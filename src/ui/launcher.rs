use eframe::egui::{self, RichText};
use snip2pin::settings::{Icon, Preferences, Theme};

use super::style::LAUNCHER;

pub enum Action {
    Capture,
    ClosePins,
    RecordShortcut,
    Appearance(Theme, Icon),
    Startup(bool),
}

pub struct Model<'a> {
    pub pin_count: usize,
    pub message: Option<&'a str>,
    pub global_shortcut: bool,
    pub preferences: Preferences,
    pub icon_texture: Option<&'a egui::TextureHandle>,
    pub recording: bool,
    pub idle: bool,
    pub demo: bool,
    pub startup: Option<bool>,
}

pub fn show(ui: &mut egui::Ui, model: Model<'_>) -> Option<Action> {
    ui.painter()
        .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
    footer(ui);
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
                    RichText::new("框选，复制，置顶。")
                        .size(LAUNCHER.heading)
                        .strong(),
                );
                ui.weak("自动复制，留在框选原位。");
                ui.add_space(LAUNCHER.section_gap);
                let capture =
                    egui::Button::new(RichText::new("截图并置顶").color(LAUNCHER.capture_text))
                        .fill(LAUNCHER.capture_fill)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(LAUNCHER.button_radius);
                if ui
                    .add_enabled_ui(model.idle && !model.recording, |ui| {
                        ui.add_sized([ui.available_width(), LAUNCHER.capture_height], capture)
                    })
                    .inner
                    .on_hover_text(
                        "拖动框选，Esc 取消。贴图可拖动、边角缩放，按 Shift 等比缩放，双击关闭。",
                    )
                    .clicked()
                {
                    action = Some(Action::Capture);
                }
                ui.add_space(LAUNCHER.action_gap);
                let binding = model.preferences.binding.label();
                let shortcut = if model.global_shortcut {
                    binding
                } else {
                    format!("窗口内 {binding}")
                };
                if action_row(
                    ui,
                    &shortcut,
                    if model.recording {
                        "取消修改"
                    } else {
                        "修改快捷键"
                    },
                    model.idle,
                )
                .clicked()
                {
                    action = Some(Action::RecordShortcut);
                }
                if model.recording {
                    ui.weak("按下新的组合键，Esc 取消。");
                }
                ui.add_space(LAUNCHER.section_gap);
                ui.separator();
                let mut theme = model.preferences.theme;
                let mut icon = model.preferences.icon;
                ui.add_enabled_ui(model.idle && !model.recording, |ui| {
                    setting_row(ui, "主题", |ui, label| {
                        egui::ComboBox::from_id_salt("theme")
                            .width(LAUNCHER.control_width)
                            .selected_text(theme.label())
                            .show_ui(ui, |ui| {
                                for choice in [Theme::System, Theme::Light, Theme::Dark] {
                                    ui.selectable_value(&mut theme, choice, choice.label());
                                }
                            })
                            .response
                            .labelled_by(label);
                    });
                    setting_row(ui, "图标", |ui, label| {
                        egui::ComboBox::from_id_salt("icon")
                            .width(LAUNCHER.control_width)
                            .selected_text(icon.label())
                            .show_ui(ui, |ui| {
                                for choice in [Icon::Auto, Icon::Line, Icon::Metal] {
                                    ui.selectable_value(&mut icon, choice, choice.label());
                                }
                            })
                            .response
                            .labelled_by(label);
                    });
                    if let Some(mut enabled) = model.startup {
                        setting_row(ui, "开机自启", |ui, label| {
                            if ui
                                .checkbox(&mut enabled, "")
                                .labelled_by(label)
                                .on_hover_text("登录后在菜单栏后台运行，默认关闭。")
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
                if model.pin_count > 0
                    && action_row(ui, &format!("{} 张贴图", model.pin_count), "关闭全部", true)
                        .clicked()
                {
                    action = Some(Action::ClosePins);
                }
                if model.demo {
                    ui.weak("演示模式：不读取屏幕。");
                }
                if let Some(message) = model.message {
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
            });
    });
    action
}

fn footer(ui: &mut egui::Ui) {
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
                        "版本与下载 ↗",
                        concat!(env!("CARGO_PKG_REPOSITORY"), "/releases"),
                    )
                    .on_hover_text("在浏览器中查看 GitHub Releases。");
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
