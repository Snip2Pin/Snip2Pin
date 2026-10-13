use eframe::egui::{self, RichText};
use snip2pin::settings::{Icon, Preferences, Theme};

pub enum Action {
    Capture,
    ClosePins,
    RecordShortcut,
    Appearance(Theme, Icon),
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
}

pub fn show(ui: &mut egui::Ui, model: Model<'_>) -> Option<Action> {
    let Model {
        pin_count,
        message,
        global_shortcut,
        preferences,
        icon_texture,
        recording,
        idle,
        demo,
    } = model;
    ui.painter()
        .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
    let mut action = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::new().inner_margin(22).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("截一张，贴在眼前。").size(24.0).strong());
                ui.add_space(10.0);
                ui.label("框选后自动置顶，拖动移动，双击关闭。");
                ui.weak("自动覆盖所有屏幕，贴图留在框选原位。");
                ui.add_space(12.0);
                ui.add_enabled_ui(idle && !recording, |ui| {
                    if ui
                        .add_sized([200.0, 40.0], egui::Button::new("截图并置顶"))
                        .clicked()
                    {
                        action = Some(Action::Capture);
                    }
                });
                ui.add_space(8.0);
                let prefix = if global_shortcut {
                    "快捷键"
                } else {
                    "窗口内快捷键"
                };
                ui.weak(format!("{prefix}  {}", preferences.binding.label()));
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(if recording {
                            "取消修改"
                        } else {
                            "修改快捷键"
                        }),
                    )
                    .clicked()
                {
                    action = Some(Action::RecordShortcut);
                }
                if recording {
                    ui.weak("按下新的组合键，Esc 取消。");
                }
                ui.add_space(10.0);
                let mut theme = preferences.theme;
                let mut icon = preferences.icon;
                ui.add_enabled_ui(idle && !recording, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("主题");
                        egui::ComboBox::from_id_salt("theme")
                            .selected_text(theme.label())
                            .show_ui(ui, |ui| {
                                for choice in [Theme::System, Theme::Light, Theme::Dark] {
                                    ui.selectable_value(&mut theme, choice, choice.label());
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("图标");
                        egui::ComboBox::from_id_salt("icon")
                            .selected_text(icon.label())
                            .show_ui(ui, |ui| {
                                for choice in [Icon::Auto, Icon::Line, Icon::Metal] {
                                    ui.selectable_value(&mut icon, choice, choice.label());
                                }
                            });
                        if let Some(texture) = icon_texture {
                            ui.image((texture.id(), egui::vec2(28.0, 28.0)));
                        }
                    });
                });
                if theme != preferences.theme || icon != preferences.icon {
                    action = Some(Action::Appearance(theme, icon));
                }
                if pin_count > 0
                    && ui
                        .small_button(format!("关闭全部贴图（{pin_count}）"))
                        .clicked()
                {
                    action = Some(Action::ClosePins);
                }
                if demo {
                    ui.weak("演示模式：使用生成的测试图，不读取屏幕。");
                }
                if let Some(message) = message {
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
            });
        });
    });
    action
}
