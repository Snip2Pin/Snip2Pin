use eframe::egui::{self, Color32, Rect, Stroke, StrokeKind};

// Copy artwork uses its own design coordinate system, independent of layout.
pub fn paint_copy_icon(ui: &egui::Ui, rect: Rect) {
    const DESIGN_SIZE: f32 = 24.0;
    const DISPLAY_SIZE: f32 = 16.0;
    const LINE_WIDTH: f32 = 1.7;
    let origin = rect.center() - egui::Vec2::splat(DISPLAY_SIZE / 2.0);
    let point = |x, y| origin + egui::vec2(x, y) * (DISPLAY_SIZE / DESIGN_SIZE);
    let stroke = Stroke::new(LINE_WIDTH * (DISPLAY_SIZE / DESIGN_SIZE), Color32::WHITE);
    ui.painter().add(egui::Shape::line(
        [
            (5.0, 15.0),
            (4.0, 15.0),
            (2.6, 14.4),
            (2.0, 13.0),
            (2.0, 5.0),
            (2.6, 3.6),
            (4.0, 3.0),
            (15.0, 3.0),
            (16.4, 3.6),
            (17.0, 5.0),
            (17.0, 6.0),
        ]
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect(),
        stroke,
    ));
    ui.painter().rect_stroke(
        Rect::from_min_max(point(7.0, 8.0), point(22.0, 21.0)),
        1,
        stroke,
        StrokeKind::Middle,
    );
    ui.painter().add(egui::Shape::line(
        [
            (8.0, 19.0),
            (12.0, 15.0),
            (15.0, 18.0),
            (17.0, 16.0),
            (21.0, 20.0),
        ]
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect(),
        stroke,
    ));
    ui.painter().circle_filled(
        point(17.5, 12.5),
        DISPLAY_SIZE / DESIGN_SIZE,
        Color32::WHITE,
    );
}
