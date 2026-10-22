mod icons;
pub mod launcher;
pub mod overlay;
pub mod pin;
pub mod screen_hint;
pub mod shortcut;
pub mod style;

fn tooltip(ui: &mut eframe::egui::Ui, text: &str) {
    let margin = eframe::egui::Frame::popup(ui.style())
        .total_margin()
        .sum()
        .x;
    let width = (ui.ctx().content_rect().width() - margin).max(0.0);
    ui.set_max_width(width.min(ui.spacing().tooltip_width));
    ui.add(eframe::egui::Label::new(text).wrap());
}
