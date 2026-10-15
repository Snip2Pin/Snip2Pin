use eframe::egui::{self, Rect};
use snip2pin::{
    geometry::Placement,
    pin::{frame_geometry, minimum_image_size},
};

#[test]
fn initial_image_keeps_original_size_and_desktop_position_when_space_exists() {
    let area = Rect::from_min_size(egui::pos2(-1280.0, 30.0), egui::vec2(1280.0, 900.0));
    let placement = Placement {
        position: [-1000.0, 200.0],
        size: [300.0, 200.0],
    };
    let layout = frame_geometry(placement, area, 1.0, 1.0);
    assert_eq!(layout.image.min, egui::pos2(-1000.0, 200.0));
    assert_eq!(layout.image.size(), egui::vec2(300.0, 200.0));
    assert!(
        layout
            .image
            .contains_rect(layout.controls.expect("controls"))
    );
    assert_eq!(placement.position, [-1000.0, 200.0]);
    assert_eq!(placement.size, [300.0, 200.0]);
}

#[test]
fn full_screen_image_keeps_its_size_without_reserving_external_button_space() {
    let area = Rect::from_min_size(egui::pos2(0.0, 30.0), egui::vec2(680.0, 420.0));
    let layout = frame_geometry(
        Placement {
            position: area.min.into(),
            size: area.size().into(),
        },
        area,
        1.0,
        1.0,
    );
    assert_eq!(layout.image, area);
    assert!(
        layout
            .image
            .contains_rect(layout.controls.expect("controls"))
    );
}

#[test]
fn oversized_images_fit_the_screen_proportionally() {
    for ratio in [1.0, 0.8, 0.625] {
        let area = Rect::from_min_size(egui::pos2(-680.0, -170.0), egui::vec2(680.0, 420.0));
        let placement = Placement {
            position: area.min.into(),
            size: [1360.0, 840.0],
        };
        let layout = frame_geometry(placement, area, ratio, ratio.recip());
        let scaled = Rect::from_min_max(area.min * ratio, area.max * ratio);
        assert!(scaled.expand(0.001).contains_rect(layout.image));
        assert!((layout.image.aspect_ratio() - 1360.0 / 840.0).abs() < 0.0001);
        assert!(
            layout
                .image
                .contains_rect(layout.controls.expect("controls"))
        );
        assert_eq!(placement.size, [1360.0, 840.0]);
    }
}

#[test]
fn tiny_images_enlarge_proportionally_to_the_native_minimum() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    let layout = frame_geometry(
        Placement {
            position: [100.0, 100.0],
            size: [8.0, 5.0],
        },
        area,
        1.0,
        1.0,
    );
    assert_eq!(layout.image.size(), minimum_image_size());
    assert_eq!(layout.image.min, egui::pos2(100.0, 100.0));
    assert!(layout.controls.is_some());
}

#[test]
fn minimum_controls_remain_reachable_at_fractional_desktop_positions() {
    let area = Rect::from_min_size(egui::pos2(80.0, 100.0), egui::vec2(680.0, 420.0));
    for scale in [1.0, 1.5, 2.0] {
        for zoom in [1.0, 1.25, 2.0] {
            let layout = frame_geometry(
                Placement {
                    position: [148.0 / scale, 142.0 / scale],
                    size: [8.0 * 680.0 / 960.0 / scale, 5.0 * 420.0 / 600.0 / scale],
                },
                Rect::from_min_max(area.min / scale, area.max / scale),
                1.0 / zoom,
                scale * zoom,
            );
            assert!(
                layout.controls.is_some(),
                "scale={scale}, zoom={zoom}, image={:?}",
                layout.image
            );
        }
    }
}

#[test]
fn extreme_aspect_ratios_never_request_a_surface_larger_than_the_screen() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    for original in [[2.0, 1200.0], [1920.0, 1.0]] {
        let placement = Placement {
            position: [100.0, 100.0],
            size: original,
        };
        let layout = frame_geometry(placement, area, 1.0, 1.0);
        assert!(area.contains_rect(layout.image));
        assert!(layout.image.width() >= minimum_image_size().x);
        assert!(layout.image.height() >= minimum_image_size().y);
        assert!(layout.controls.is_some());
        assert_eq!(placement.size, original);
    }
}

#[test]
fn initial_layout_moves_only_as_far_as_needed_to_fit() {
    let area = Rect::from_min_size(egui::pos2(0.0, 30.0), egui::vec2(680.0, 420.0));
    let layout = frame_geometry(
        Placement {
            position: [640.0, 400.0],
            size: [200.0, 100.0],
        },
        area,
        1.0,
        1.0,
    );
    assert_eq!(layout.image.min, egui::pos2(480.0, 350.0));
    assert_eq!(layout.image.size(), egui::vec2(200.0, 100.0));
    assert!(layout.controls.is_some());
}

#[test]
fn initial_fractional_dpi_rounds_position_without_stretching_the_image() {
    let area = Rect::from_min_size(egui::pos2(-1000.0, -500.0), egui::vec2(1000.0, 800.0));
    let layout = frame_geometry(
        Placement {
            position: [-700.25, -200.25],
            size: [193.0, 121.0],
        },
        area,
        0.8,
        1.25,
    );
    assert!((layout.image.width() - 193.0 * 0.8).abs() < 0.001);
    assert!((layout.image.height() - 121.0 * 0.8).abs() < 0.001);
    assert!((layout.image.min.x * 1.25 - (layout.image.min.x * 1.25).round()).abs() < 0.001);
    assert!(layout.controls.is_some());
}

#[test]
fn incompatible_tiny_axis_does_not_enlarge_an_already_visible_long_axis() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    for (size, expected) in [([2.0, 200.0], [80.0, 200.0]), ([300.0, 2.0], [300.0, 50.0])] {
        let layout = frame_geometry(
            Placement {
                position: [100.0, 100.0],
                size,
            },
            area,
            1.0,
            1.0,
        );
        assert_eq!(layout.image.size(), egui::Vec2::from(expected));
        assert_eq!(layout.image.min, egui::pos2(100.0, 100.0));
        assert!(layout.controls.is_some());
    }
}
