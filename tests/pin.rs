use eframe::egui::{self, Rect, Vec2};
use snip2pin::{
    geometry::Placement,
    pin::{self, Coordinates, Drag, Preparation, Prepared, ToolbarSide},
};

#[test]
fn controls_remain_visible_at_every_edge_and_corner_without_resizing_or_overlap() {
    for origin in [egui::pos2(0.0, 28.0), egui::pos2(-2560.0, -172.0)] {
        for ratio in [1.0, 0.8, 0.625] {
            for size in [[1.0, 1.0], [12.0, 8.0], [193.0, 121.0], [680.0, 420.0]] {
                let area = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
                let frame = pin::frame_geometry(
                    Placement {
                        position: (origin + Vec2::splat(100.0)).into(),
                        size,
                    },
                    area,
                    ratio,
                    ratio.recip(),
                );
                let inset = egui::vec2(4.0, 7.0) * ratio;
                let scaled = frame.controls_area;
                for target in [
                    scaled.left_top(),
                    scaled.center_top(),
                    scaled.right_top(),
                    scaled.right_center(),
                    scaled.right_bottom(),
                    scaled.center_bottom(),
                    scaled.left_bottom(),
                    scaled.left_center(),
                ] {
                    let direction = (target - scaled.center()).normalized();
                    let candidate = target + direction * scaled.size().length()
                        - frame.image.center().to_vec2()
                        - inset;
                    let position = frame.drag_position(candidate, inset, &[area], ratio);
                    let inner = Rect::from_min_size(position + inset, frame.window.size());
                    let mut moved = frame;
                    moved.place_controls(inner, &[area], area, ratio);
                    assert_eq!(
                        moved.image, frame.image,
                        "switching sides does not move or resize pixels"
                    );
                    let local_bounds = Rect::from_min_size(egui::Pos2::ZERO, inner.size());
                    for button in [moved.toolbar_rects().0, moved.toolbar_rects().1] {
                        assert!(local_bounds.expand(0.001).contains_rect(button));
                        assert!(
                            scaled
                                .expand(0.001)
                                .contains_rect(button.translate(inner.min.to_vec2())),
                            "buttons stay in work area for {size:?}, {target:?}, {:?}",
                            moved.toolbar_side
                        );
                        assert!(!button.intersects(moved.image));
                    }
                }
            }
        }
    }
}

#[test]
fn dragging_can_cross_a_monitor_gap_without_being_limited_by_the_old_monitor() {
    let left = Rect::from_min_size(egui::pos2(-680.0, -172.0), egui::vec2(680.0, 420.0));
    let right = Rect::from_min_size(egui::pos2(500.0, 28.0), left.size());
    let frame = pin::frame_geometry(
        Placement {
            position: [-500.0, -50.0],
            size: [193.0, 121.0],
        },
        left,
        1.0,
        1.0,
    );
    let target = right.min + Vec2::splat(100.0);
    let candidate = target - frame.image.min.to_vec2();
    let position = frame.drag_position(candidate, Vec2::ZERO, &[left, right], 1.0);
    assert_eq!(position, candidate);
    let mut moved = frame;
    moved.place_controls(
        Rect::from_min_size(position, frame.window.size()),
        &[left, right],
        left,
        1.0,
    );
    assert_eq!(moved.controls_area, right);
}

#[test]
fn right_edge_overflow_places_controls_to_the_left_of_the_image() {
    for origin in [egui::pos2(0.0, 28.0), egui::pos2(-680.0, -172.0)] {
        for ratio in [1.0, 0.8] {
            let screen = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
            let mut frame = pin::frame_geometry(
                Placement {
                    position: [screen.max.x - 50.0, screen.min.y + 100.0],
                    size: [193.0, 121.0],
                },
                screen,
                ratio,
                ratio.recip(),
            );
            frame.place_controls(frame.window, &[screen], screen, ratio);
            let image = frame.image.translate(frame.window.min.to_vec2());
            for button in [frame.toolbar_rects().0, frame.toolbar_rects().1] {
                let button = button.translate(frame.window.min.to_vec2());
                assert!(
                    button.max.x < image.min.x,
                    "right overflow should put controls on the left"
                );
                assert!(frame.controls_area.contains_rect(button));
                assert!(!button.intersects(image));
            }
        }
    }
}

#[test]
fn tall_pins_keep_a_reachable_row_before_and_after_moving_off_the_top() {
    let display = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    let usable = Rect::from_min_max(egui::pos2(0.0, 28.0), display.max);
    let mut frame = pin::frame_geometry(
        Placement {
            position: [0.0, 0.0],
            size: display.size().into(),
        },
        usable,
        1.0,
        1.0,
    );
    frame.place_controls(frame.window, &[display], usable, 1.0);
    assert!(
        frame.toolbar_side != ToolbarSide::Bottom,
        "a tall initial pin needs its visible upper row"
    );
    let position = frame.window.min - egui::vec2(0.0, display.height() / 2.0);
    frame.place_controls(
        Rect::from_min_size(position, frame.window.size()),
        &[display],
        usable,
        1.0,
    );
    assert!(
        (frame.toolbar_side == ToolbarSide::Bottom),
        "the lower row becomes reachable when the image leaves the top"
    );
    for button in [frame.toolbar_rects().0, frame.toolbar_rects().1] {
        assert!(display.contains_rect(button.translate(position.to_vec2())));
    }
}

#[test]
fn small_images_can_leave_the_work_area_while_controls_avoid_system_bars() {
    let display = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    let usable = Rect::from_min_max(egui::pos2(0.0, 28.0), display.max);
    let mut frame = pin::frame_geometry(
        Placement {
            position: [100.0, 28.0],
            size: [12.0, 8.0],
        },
        usable,
        1.0,
        1.0,
    );
    frame.place_controls(frame.window, &[usable], usable, 1.0);
    let position = frame.drag_position(
        egui::pos2(frame.window.min.x, -display.height()),
        Vec2::ZERO,
        &[usable],
        1.0,
    );
    assert!(position.y + frame.image.max.y < usable.min.y);
    frame.place_controls(
        Rect::from_min_size(position, frame.window.size()),
        &[usable],
        usable,
        1.0,
    );
    let (copy, close) = frame.toolbar_rects();
    for button in [copy, close] {
        assert!(usable.contains_rect(button.translate(position.to_vec2())));
    }
}

#[test]
fn upward_boundary_depends_on_lower_controls_instead_of_image_top() {
    for origin in [egui::pos2(0.0, 28.0), egui::pos2(-680.0, -172.0)] {
        for ratio in [1.0, 0.8] {
            let screen = Rect::from_min_size(origin, egui::vec2(680.0, 420.0));
            let frame = pin::frame_geometry(
                Placement {
                    position: (origin + Vec2::splat(100.0)).into(),
                    size: [193.0, 121.0],
                },
                screen,
                ratio,
                ratio.recip(),
            );
            let inset = egui::vec2(4.0, 7.0) * ratio;
            for distance in [frame.image.height() / 2.0, frame.image.height() * 2.0] {
                let candidate = egui::pos2(
                    frame.window.min.x,
                    frame.controls_area.min.y - frame.image.min.y - distance,
                ) - inset;
                let position = frame.drag_position(candidate, inset, &[screen], ratio);
                let inner = Rect::from_min_size(position + inset, frame.window.size());
                let image = frame.image.translate(inner.min.to_vec2());
                assert!(image.min.y < frame.controls_area.min.y);
                assert!((image.size() - frame.image.size()).length() < 0.001);
                let mut moved = frame;
                moved.place_controls(inner, &[screen], screen, ratio);
                assert!((moved.toolbar_side == ToolbarSide::Bottom));
                for button in [moved.toolbar_rects().0, moved.toolbar_rects().1] {
                    assert!(
                        moved
                            .controls_area
                            .expand(0.001)
                            .contains_rect(button.translate(inner.min.to_vec2()))
                    );
                }
                if distance > frame.image.height() {
                    assert!(
                        image.max.y < frame.controls_area.min.y,
                        "even the entire image may leave the usable area"
                    );
                }
            }
        }
    }
}

#[test]
fn an_offscreen_image_keeps_the_work_area_of_its_nearest_monitor() {
    let left = Rect::from_min_size(egui::pos2(-680.0, -172.0), egui::vec2(680.0, 420.0));
    let right = Rect::from_min_size(egui::pos2(0.0, 28.0), left.size());
    let mut frame = pin::frame_geometry(
        Placement {
            position: [-500.0, -50.0],
            size: [193.0, 121.0],
        },
        left,
        1.0,
        1.0,
    );
    let inner = frame.window.translate(egui::vec2(0.0, -left.height()));
    frame.place_controls(inner, &[right, left], right, 1.0);
    assert_eq!(frame.controls_area, left);
}

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
            assert!((frame.toolbar_side == ToolbarSide::Bottom));
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
            assert!(frame.toolbar_side != ToolbarSide::Bottom);
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
