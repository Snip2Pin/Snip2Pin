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

#[test]
fn initial_fit_shrinks_only_when_no_side_can_fit_the_complete_image_and_controls() {
    for origin in [egui::pos2(0.0, 28.0), egui::pos2(-1920.0, -180.0)] {
        for (ratio, grid) in [(1.0, 1.0), (0.8, 1.25), (0.5, 2.0)] {
            let area = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
            let visible = Rect::from_min_max(area.min * ratio, area.max * ratio);
            for size in [egui::vec2(680.0, 420.0), egui::vec2(1500.0, 1200.0)] {
                let source = Placement {
                    position: origin.into(),
                    size: size.into(),
                };
                let frame = snip2pin::pin::host_frame(frame_geometry(source, area, ratio, grid));
                let image = frame.image.translate(frame.window.min.to_vec2());
                assert!(
                    visible.contains_rect(image),
                    "image {image:?} must fit {visible:?}"
                );
                assert!(frame.image.width() < size.x * ratio);
                assert!((frame.image.aspect_ratio() - size.x / size.y).abs() < 0.001);
                for button in [frame.toolbar_rects().0, frame.toolbar_rects().1] {
                    assert!(visible.contains_rect(button.translate(frame.window.min.to_vec2())));
                    assert!(!button.intersects(frame.image));
                }
                let moved = frame.moved_to(visible.min - egui::vec2(400.0, 0.0), &[area], ratio);
                assert_eq!(
                    moved.image.size(),
                    frame.image.size(),
                    "manual movement must not fit again"
                );
                let manual = moved.resized_to(
                    Rect::from_min_size(image.min, size * ratio * 2.0),
                    &[area],
                    ratio,
                );
                assert_eq!(
                    manual.image.size(),
                    size * ratio * 2.0,
                    "manual resize must not fit again"
                );
                assert_eq!(
                    source.size,
                    [size.x, size.y],
                    "original placement remains unchanged"
                );
            }
        }
    }
}

#[test]
fn initial_fit_repositions_without_shrinking_when_original_size_can_fit() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    let frame = frame_geometry(
        Placement {
            position: [500.0, 300.0],
            size: [600.0, 380.0],
        },
        area,
        1.0,
        1.0,
    );
    assert_eq!(frame.image.size(), egui::vec2(600.0, 380.0));
    assert!(area.contains_rect(frame.image.translate(frame.window.min.to_vec2())));
    for button in [frame.toolbar_rects().0, frame.toolbar_rects().1] {
        assert!(area.contains_rect(button.translate(frame.window.min.to_vec2())));
    }
}
