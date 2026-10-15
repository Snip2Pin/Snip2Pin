use eframe::egui::{self, Rect};
use snip2pin::{
    geometry::Placement,
    pin::{ToolbarSide, frame_geometry},
};

#[test]
fn full_height_image_uses_free_side_without_being_pushed_down() {
    let screen = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let frame = frame_geometry(
        Placement {
            position: [600.0, 1.0],
            size: [598.0, 797.0],
        },
        screen,
        1.0,
        1.0,
    );
    assert_eq!(
        frame.image.min + frame.window.min.to_vec2(),
        egui::pos2(600.0, 1.0)
    );
    assert_eq!(frame.image.size(), egui::vec2(598.0, 797.0));
    assert_eq!(frame.toolbar_side, ToolbarSide::Left);
    for button in [frame.toolbar_rects().0, frame.toolbar_rects().1] {
        assert!(screen.contains_rect(button.translate(frame.window.min.to_vec2())));
    }
}
