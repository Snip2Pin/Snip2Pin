use eframe::egui::{self, Rect};
use snip2pin::{geometry::Placement, pin::frame_geometry};

#[test]
fn initial_image_keeps_original_size_and_desktop_position_when_space_exists() {
    let area = Rect::from_min_size(egui::pos2(-1280.0, 30.0), egui::vec2(1280.0, 900.0));
    let placement = Placement {
        position: [-1000.0, 200.0],
        size: [300.0, 200.0],
    };
    let layout = frame_geometry(placement, area, 1.0);
    assert_eq!(layout.image.min, egui::pos2(-1000.0, 200.0));
    assert_eq!(layout.image.size(), egui::vec2(300.0, 200.0));
    assert!(
        layout
            .image
            .contains_rect(layout.controls.expect("controls").bounds)
    );
    assert_eq!(placement.position, [-1000.0, 200.0]);
    assert_eq!(placement.size, [300.0, 200.0]);
}

#[test]
fn narrow_image_keeps_original_size_and_position_without_enlarging_for_controls() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0));
    let placement = Placement {
        position: [100.0, 100.0],
        size: [20.0, 120.0],
    };

    let layout = frame_geometry(placement, area, 1.0);

    assert_eq!(
        layout.image.size(),
        egui::Vec2::from(placement.size),
        "a narrow screenshot that fits the work area must preserve its content scale"
    );
    assert_eq!(layout.image.min, egui::Pos2::from(placement.position));
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
    );
    assert_eq!(layout.image, area);
    assert!(
        layout
            .image
            .contains_rect(layout.controls.expect("controls").bounds)
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
        let layout = frame_geometry(placement, area, ratio);
        let scaled = Rect::from_min_max(area.min * ratio, area.max * ratio);
        assert!(scaled.expand(0.001).contains_rect(layout.image));
        assert!((layout.image.aspect_ratio() - 1360.0 / 840.0).abs() < 0.0001);
        assert!(
            layout
                .image
                .contains_rect(layout.controls.expect("controls").bounds)
        );
        assert_eq!(placement.size, [1360.0, 840.0]);
    }
}

#[test]
fn fitting_extreme_strips_preserves_aspect_without_an_application_minimum() {
    let area = Rect::from_min_size(egui::pos2(0.0, 30.0), egui::vec2(1000.0, 1000.0));
    for ratio in [1.0, 0.8, 0.5] {
        for size in [[2.0, 1080.0], [1080.0, 2.0], [4.0, 4096.0]] {
            let placement = Placement {
                position: area.min.into(),
                size,
            };
            let image = frame_geometry(placement, area, ratio).image;
            assert!(image.size().min_elem() / ratio < 2.0);
            assert!((image.width() / size[0] - image.height() / size[1]).abs() < 0.00001);
            assert!(image.width() <= size[0] * ratio && image.height() <= size[1] * ratio);
            assert!(image.intersects(Rect::from_min_max(area.min * ratio, area.max * ratio)));
        }
    }
}

#[test]
fn images_that_fit_preserve_size_aspect_and_origin_even_when_controls_do_not_fit() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    for width in [2.0, 12.0, 26.0, 40.0, 55.0, 56.0, 79.0, 85.0, 86.0, 160.0] {
        for height in [2.0, 20.0, 50.0, 120.0, 200.0] {
            let placement = Placement {
                position: [100.0, 100.0],
                size: [width, height],
            };
            let layout = frame_geometry(placement, area, 1.0);

            assert_eq!(layout.image.min, egui::Pos2::from(placement.position));
            assert_eq!(layout.image.size(), egui::Vec2::from(placement.size));
            assert_eq!(layout.image.aspect_ratio(), width / height);
            let expected_controls = if width >= 86.0 && height >= 26.0 {
                Some((egui::vec2(86.0, 26.0), 3))
            } else if width >= 56.0 && height >= 26.0 {
                Some((egui::vec2(56.0, 26.0), 2))
            } else if width >= 26.0 && height >= 26.0 {
                Some((egui::vec2(26.0, 26.0), 1))
            } else {
                None
            };
            assert_eq!(
                layout
                    .controls
                    .map(|controls| (controls.bounds.size(), controls.count)),
                expected_controls,
                "controls must adapt without enlarging {width}x{height}"
            );
            if let Some(controls) = layout.controls {
                assert!(layout.image.contains_rect(controls.bounds));
                assert_eq!(
                    controls.bounds.height(),
                    26.0,
                    "controls must remain horizontal"
                );
            }
            assert_eq!(placement.size, [width, height]);
        }
    }
}

#[test]
fn tiny_images_keep_their_content_scale_across_dpi_and_zoom() {
    let area = Rect::from_min_size(egui::pos2(80.0, 100.0), egui::vec2(680.0, 420.0));
    for scale in [1.0, 1.5, 2.0] {
        for zoom in [1.0, 1.25, 2.0] {
            let placement = Placement {
                position: [148.0 / scale, 142.0 / scale],
                size: [8.0 * 680.0 / 960.0 / scale, 5.0 * 420.0 / 600.0 / scale],
            };
            let layout = frame_geometry(
                placement,
                Rect::from_min_max(area.min / scale, area.max / scale),
                1.0 / zoom,
            );
            let expected_size = egui::Vec2::from(placement.size) / zoom;
            assert!((layout.image.width() - expected_size.x).abs() < 0.001);
            assert!((layout.image.height() - expected_size.y).abs() < 0.001);
            let expected_position = egui::Pos2::from(placement.position) / zoom;
            assert_eq!(layout.image.min, expected_position);
            assert!(layout.controls.is_none());
        }
    }
}

#[test]
fn oversized_extreme_aspect_ratios_only_shrink_proportionally() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    for original in [[2.0, 1200.0], [1920.0, 1.0], [1360.0, 840.0]] {
        let placement = Placement {
            position: [100.0, 100.0],
            size: original,
        };
        let layout = frame_geometry(placement, area, 1.0);
        assert!(area.contains_rect(layout.image));
        let expected_scale = (area.width() / original[0]).min(area.height() / original[1]);
        assert!(expected_scale < 1.0);
        assert!((layout.image.width() - original[0] * expected_scale).abs() < 0.001);
        assert!((layout.image.height() - original[1] * expected_scale).abs() < 0.001);
        assert!(
            (layout.image.width() / original[0] - layout.image.height() / original[1]).abs()
                < 0.00001,
            "an oversized {original:?} screenshot must not stretch"
        );
        if let Some(controls) = layout.controls {
            assert!(layout.image.contains_rect(controls.bounds));
        }
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
    );
    assert_eq!(layout.image.min, egui::pos2(480.0, 350.0));
    assert_eq!(layout.image.size(), egui::vec2(200.0, 100.0));
    assert!(layout.controls.is_some());
}

#[test]
fn initial_fractional_dpi_preserves_position_without_rounding_or_stretching() {
    let area = Rect::from_min_size(egui::pos2(-1000.0, -500.0), egui::vec2(1000.0, 800.0));
    let layout = frame_geometry(
        Placement {
            position: [-700.25, -200.25],
            size: [193.0, 121.0],
        },
        area,
        0.8,
    );
    assert!((layout.image.width() - 193.0 * 0.8).abs() < 0.001);
    assert!((layout.image.height() - 121.0 * 0.8).abs() < 0.001);
    assert_eq!(layout.image.min, egui::pos2(-700.25, -200.25) * 0.8);
    assert!(layout.controls.is_some());
}

#[test]
fn a_tiny_axis_never_enlarges_either_axis_for_controls() {
    let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(680.0, 420.0));
    for size in [[2.0, 200.0], [300.0, 2.0]] {
        let layout = frame_geometry(
            Placement {
                position: [100.0, 100.0],
                size,
            },
            area,
            1.0,
        );
        assert_eq!(layout.image.size(), egui::Vec2::from(size));
        assert_eq!(layout.image.min, egui::pos2(100.0, 100.0));
        assert!(layout.controls.is_none());
    }
}
