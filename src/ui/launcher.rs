use eframe::egui::{self, Color32, RichText};
use snipsprig::capture::Display;

pub enum Action {
    Capture,
    ClosePins,
}

pub fn show(
    ui: &mut egui::Ui,
    displays: &[Display],
    display_index: &mut usize,
    pin_count: usize,
    message: Option<&str>,
    global_shortcut: bool,
    demo: bool,
) -> Option<Action> {
    ui.painter()
        .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
    let mut action = None;
    egui::Frame::new().inner_margin(22).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("截一张，贴在眼前。").size(24.0).strong());
            ui.add_space(10.0);
            ui.label("框选后自动置顶，拖动移动，双击关闭。");
            if displays.len() > 1 {
                egui::ComboBox::from_id_salt("display")
                    .selected_text(&displays[*display_index].name)
                    .show_ui(ui, |ui| {
                        for (index, display) in displays.iter().enumerate() {
                            ui.selectable_value(display_index, index, &display.name);
                        }
                    });
            }
            ui.add_space(12.0);
            if ui
                .add_sized([200.0, 40.0], egui::Button::new("截图并置顶"))
                .clicked()
            {
                action = Some(Action::Capture);
            }
            ui.add_space(8.0);
            let prefix = if global_shortcut {
                "快捷键"
            } else {
                "窗口内快捷键"
            };
            ui.weak(format!("{prefix}  {}", crate::platform::SHORTCUT));
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
