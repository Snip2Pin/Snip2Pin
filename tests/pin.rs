use eframe::egui::{self, Rect, Vec2};
use snip2pin::{
    geometry::Placement,
    pin::{self, Coordinates, Drag, Preparation, Prepared},
};

#[test]
fn dragging_can_put_the_image_at_the_top_while_margins_leave_the_work_area() {
    for origin in [egui::pos2(0.0, 28.0), egui::pos2(-680.0, -172.0)] {
        for (ratio, grid) in [(1.0, 1.0), (0.8, 1.25)] {
            let screen = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
            let layout = pin::frame_geometry(
                Placement {
                    position: (origin + egui::vec2(100.0, 100.0)).into(),
                    size: [193.0, 121.0],
                },
                screen,
                ratio,
                grid,
            );
            let inset = egui::vec2(4.0, 7.0) * ratio;
            let outer = layout.window.translate(-inset);
            let press = layout.image.center();
            let mut drag = Drag::default();
            let coordinates = Coordinates::new(1.0, 1.0, 1.0);
            drag.begin(press, layout.window, outer, &coordinates);
            let target_image = origin * ratio;
            let cursor = target_image + press.to_vec2() - layout.image.min.to_vec2();
            let moved = drag
                .move_to(cursor.into(), &coordinates)
                .expect("user moves the pointer");
            let inner = Rect::from_min_size(moved + inset, layout.window.size());
            let image = layout.image.translate(inner.min.to_vec2());
            assert!(image.min.distance(target_image) < 0.001);
            assert!(
                inner.min.y < screen.min.y * ratio,
                "transparent upper row may leave the work area"
            );
            assert!((image.size() - layout.image.size()).length() < 0.001);
            let mut frame = layout;
            frame.place_controls(inner, &[screen], screen, ratio);
            assert!(frame.toolbar_below);
            let (copy, close) = frame.toolbar_rects();
            assert!(copy.min.y > frame.image.max.y && close.min.y > frame.image.max.y);
            assert_eq!(
                drag.move_to(cursor.into(), &coordinates),
                None,
                "a stationary pointer must not cause retries"
            );
            let cursor = cursor + Vec2::splat(pin::METRICS.toolbar_height() * 3.0);
            let moved = drag
                .move_to(cursor.into(), &coordinates)
                .expect("drag away from top");
            frame.place_controls(
                Rect::from_min_size(moved + inset, layout.window.size()),
                &[screen],
                screen,
                ratio,
            );
            assert!(!frame.toolbar_below);
            drag.end();
            assert_eq!(
                drag.move_to((cursor + Vec2::splat(100.0)).into(), &coordinates),
                None
            );
        }
    }
}

#[test]
fn drag_anchor_survives_a_change_in_zoom_or_native_dpi() {
    let from = Coordinates::new(1.0, 1.0, 1.0);
    let to = Coordinates::new(1.0, 2.0, 2.0);
    let inner = Rect::from_min_size(egui::pos2(-500.0, 100.0), egui::vec2(200.0, 200.0));
    let outer = inner.translate(-egui::vec2(4.0, 7.0));
    let press = egui::pos2(80.0, 100.0);
    let mut drag = Drag::default();
    drag.begin(press, inner, outer, &from);
    let native_cursor = egui::pos2(-200.0, -100.0);
    let expected_native_origin = native_cursor - (press.to_vec2() + (inner.min - outer.min));
    assert_eq!(
        drag.move_to(native_cursor.into(), &to),
        Some(to.cursor(expected_native_origin.into()))
    );
}

#[test]
fn native_geometry_mismatch_only_changes_local_image_offset_once() {
    let screen = Rect::from_min_size(egui::pos2(-680.0, -172.0), egui::vec2(680.0, 420.0));
    let frame = pin::frame_geometry(
        Placement {
            position: [-500.0, 100.0],
            size: [193.0, 121.0],
        },
        screen,
        1.0,
        1.0,
    );
    let actual = frame.window.translate(egui::vec2(4.0, 3.0));
    let mut preparation = Preparation::default();
    assert!(matches!(
        preparation.update(frame, Some(actual), Some(actual), 1.0),
        Prepared::Initialize { .. }
    ));
    for first in [true, false, false] {
        let Prepared::Paint {
            frame: painted,
            first: actual_first,
        } = preparation.update(frame, Some(actual), Some(actual), 1.0)
        else {
            panic!("initialization must not repeat");
        };
        assert_eq!(actual_first, first);
        assert_eq!(painted.image.size(), frame.image.size());
        assert_eq!(
            painted.image.min + actual.min.to_vec2(),
            frame.image.min + frame.window.min.to_vec2()
        );
    }
}
