use eframe::egui::{self, Color32, RichText};
use snipsprig::settings::Binding;

pub enum Action {
    Capture,
    ClosePins,
    RecordShortcut,
}

pub struct Model<'a> {
    pub pin_count: usize,
    pub message: Option<&'a str>,
    pub global_shortcut: bool,
    pub binding: Binding,
    pub recording: bool,
    pub idle: bool,
    pub demo: bool,
}

pub fn show(ui: &mut egui::Ui, model: Model<'_>) -> Option<Action> {
    let Model {
        pin_count,
        message,
        global_shortcut,
        binding,
        recording,
        idle,
        demo,
    } = model;
    ui.painter()
        .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
    let mut action = None;
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
            ui.weak(format!("{prefix}  {}", binding.label()));
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
                ui.colored_label(Color32::from_rgb(170, 82, 58), message);
            }
        });
    });
    action
}
