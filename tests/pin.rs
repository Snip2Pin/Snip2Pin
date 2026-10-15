use eframe::egui::{self, Rect};
use snip2pin::pin::{Coordinates, METRICS, minimum_image_size, toolbar_geometry};

fn area() -> Rect {
    Rect::from_min_size(egui::pos2(0.0, 30.0), egui::vec2(680.0, 420.0))
}

fn controls(image: Rect, area: Rect) -> Rect {
    let row = toolbar_geometry(image, &[area], area, 1.0).expect("visible controls");
    assert!(area.contains_rect(row));
    assert!(image.contains_rect(row));
    assert_eq!(row.size(), METRICS.toolbar_size());
    row
}

#[test]
fn controls_default_to_the_image_top_right_inside_resize_edges() {
    let image = Rect::from_min_size(egui::pos2(100.0, 120.0), egui::vec2(200.0, 100.0));
    let row = controls(image, area());
    assert_eq!(row.max.x, image.max.x - METRICS.inset);
    assert_eq!(row.min.y, image.min.y + METRICS.inset);
}

#[test]
fn right_and_top_occlusion_move_controls_into_the_visible_image() {
    let area = area();
    let right = Rect::from_min_size(egui::pos2(550.0, 150.0), egui::vec2(200.0, 100.0));
    assert_eq!(controls(right, area).max.x, area.max.x - METRICS.inset);
    let top = Rect::from_min_size(egui::pos2(100.0, -50.0), egui::vec2(200.0, 200.0));
    assert_eq!(controls(top, area).min.y, area.min.y + METRICS.inset);
    let corner = Rect::from_min_size(egui::pos2(550.0, -50.0), egui::vec2(200.0, 200.0));
    let row = controls(corner, area);
    assert_eq!(row.max.x, area.max.x - METRICS.inset);
    assert_eq!(row.min.y, area.min.y + METRICS.inset);
}

#[test]
fn controls_stay_inside_images_clipped_at_the_left_or_bottom() {
    let area = area();
    for image in [
        Rect::from_min_size(egui::pos2(-100.0, 100.0), egui::vec2(200.0, 100.0)),
        Rect::from_min_size(egui::pos2(100.0, 350.0), egui::vec2(200.0, 200.0)),
        Rect::from_min_size(egui::pos2(-100.0, 350.0), egui::vec2(200.0, 200.0)),
    ] {
        controls(image, area);
    }
}

#[test]
fn minimum_window_size_fits_both_buttons_and_resize_insets() {
    let area = area();
    let image = Rect::from_min_size(egui::pos2(100.0, 120.0), minimum_image_size());
    let row = controls(image, area);
    assert_eq!(row, image.shrink(METRICS.inset));
}

#[test]
fn fractional_minimum_controls_reject_real_size_or_visible_area_shortfalls() {
    for origin in [
        egui::pos2(148.0, 142.0),
        egui::pos2(-148.0, -142.0),
        egui::pos2(-16384.0, 8192.0),
        egui::pos2(32768.0, -16384.0),
        egui::pos2(1_000_000.0, 100.0),
    ] {
        for scale in [1.0, 1.5, 2.0] {
            for zoom in [1.0, 1.25, 2.0] {
                let position = origin / (scale * zoom);
                for axis in [egui::Vec2::X, egui::Vec2::Y] {
                    for margin in [-1.0, -0.25, -0.1, 0.0, 0.1, 0.25, 1.0] {
                        let image =
                            Rect::from_min_size(position, minimum_image_size() + axis * margin);
                        let area = image.expand(100.0);
                        let row = toolbar_geometry(image, &[area], area, 1.0);
                        assert_eq!(
                            row.is_some(),
                            margin >= 0.0,
                            "image size: origin={origin:?}, scale={scale}, zoom={zoom}, axis={axis:?}, margin={margin}"
                        );
                        if let Some(row) = row {
                            assert!(image.contains_rect(row));
                        }
                    }
                    let image = Rect::from_min_size(position, minimum_image_size());
                    for deficit in [0.1, 0.25, 1.0] {
                        let area = Rect::from_min_max(image.min, image.max - axis * deficit);
                        assert!(
                            toolbar_geometry(image, &[area], area, 1.0).is_none(),
                            "visible area: origin={origin:?}, scale={scale}, zoom={zoom}, axis={axis:?}, deficit={deficit}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn insufficient_visible_space_returns_none_without_relocating_the_image() {
    let area = area();
    for image in [
        Rect::from_min_size(egui::pos2(630.0, 150.0), egui::vec2(200.0, 100.0)),
        Rect::from_min_size(egui::pos2(100.0, 430.0), egui::vec2(200.0, 100.0)),
        Rect::from_min_size(egui::pos2(800.0, 500.0), egui::vec2(200.0, 100.0)),
    ] {
        assert!(toolbar_geometry(image, &[area], area, 1.0).is_none());
    }
}

#[test]
fn negative_monitor_origins_and_fractional_dpi_use_one_coordinate_space() {
    let left = Rect::from_min_size(egui::pos2(-1280.0, -170.0), egui::vec2(1280.0, 900.0));
    let right = area();
    for ratio in [1.0, 0.8, 0.625] {
        let image = Rect::from_min_size(
            egui::pos2(-1000.0, 100.0) * ratio,
            egui::vec2(200.0, 100.0) * ratio,
        );
        let scaled = Rect::from_min_max(left.min * ratio, left.max * ratio);
        let row = toolbar_geometry(image, &[right, left], right, ratio).expect("left display");
        assert!(scaled.contains_rect(row));
        assert!(image.contains_rect(row));
        assert_eq!(toolbar_geometry(image, &[], left, ratio), Some(row));
    }
}

#[test]
fn controls_use_a_roomy_intersection_when_the_closest_screen_only_shows_a_strip() {
    let left = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1920.0, 1040.0));
    let right = Rect::from_min_max(egui::pos2(1920.0, 0.0), egui::pos2(3200.0, 720.0));
    let image = Rect::from_min_max(egui::pos2(1800.0, 700.0), egui::pos2(2220.0, 880.0));
    for screens in [[left, right], [right, left]] {
        let row = toolbar_geometry(image, &screens, right, 1.0).expect("left visible image");
        assert!(left.contains_rect(row));
        assert!(image.contains_rect(row));
    }
}

#[test]
fn controls_choose_the_nearest_valid_position_to_the_image_top_right() {
    let left = Rect::from_min_max(egui::pos2(-700.0, 0.0), egui::pos2(-100.0, 500.0));
    let right = Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(700.0, 600.0));
    let image = Rect::from_min_max(egui::pos2(-200.0, 150.0), egui::pos2(200.0, 350.0));
    for screens in [[left, right], [right, left]] {
        let row = toolbar_geometry(image, &screens, left, 1.0).expect("right visible image");
        assert!(right.contains_rect(row));
        assert_eq!(row.max.x, image.max.x - METRICS.inset);
        assert_eq!(row.min.y, image.min.y + METRICS.inset);
    }
}

#[test]
fn viewport_coordinate_conversion_keeps_native_and_capture_units_distinct() {
    let coordinates = Coordinates::new(2.0, 1.25, 2.5);
    assert_eq!(coordinates.ratio, 0.8);
    assert_eq!(
        coordinates.grid,
        if cfg!(target_os = "macos") { 1.25 } else { 2.5 }
    );
}
