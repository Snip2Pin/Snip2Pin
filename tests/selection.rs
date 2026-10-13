use image::{Rgba, RgbaImage};
use snip2pin::{
    capture,
    geometry::{self, PixelRect},
    session::CaptureGate,
};

#[test]
fn all_drag_directions_select_identical_retina_pixels() {
    let expected = PixelRect {
        x: 20,
        y: 40,
        width: 80,
        height: 120,
    };
    for (start, end) in [
        ([10.0, 20.0], [50.0, 80.0]),
        ([50.0, 80.0], [10.0, 20.0]),
        ([50.0, 20.0], [10.0, 80.0]),
        ([10.0, 80.0], [50.0, 20.0]),
    ] {
        assert_eq!(
            geometry::selection(start, end, [100.0, 100.0], [200, 200]),
            Some(expected)
        );
    }
}

#[test]
fn crossing_screen_edges_clamps_without_overflow() {
    assert_eq!(
        geometry::selection([-40.0, -20.0], [500.0, 500.0], [100.0, 100.0], [200, 300]),
        Some(PixelRect {
            x: 0,
            y: 0,
            width: 200,
            height: 300
        })
    );
}

#[test]
fn invalid_and_empty_selections_are_rejected() {
    for (start, end, viewport) in [
        ([0.0, 0.0], [0.0, 30.0], [100.0, 100.0]),
        ([f64::NAN, 0.0], [10.0, 30.0], [100.0, 100.0]),
        ([0.0, 0.0], [10.0, 30.0], [0.0, 100.0]),
    ] {
        assert!(geometry::selection(start, end, viewport, [100, 100]).is_none());
    }
}

#[test]
fn cropped_pixels_are_the_exact_selected_region() {
    let image = RgbaImage::from_fn(10, 10, |x, y| Rgba([x as u8, y as u8, 0, 255]));
    let cropped = capture::crop(
        &image,
        PixelRect {
            x: 2,
            y: 3,
            width: 4,
            height: 2,
        },
    )
    .expect("valid crop");
    assert_eq!(cropped.dimensions(), (4, 2));
    assert_eq!(cropped.get_pixel(0, 0), &Rgba([2, 3, 0, 255]));
    assert_eq!(cropped.get_pixel(3, 1), &Rgba([5, 4, 0, 255]));
}

#[test]
fn malformed_crop_cannot_panic_or_read_outside_image() {
    let image = RgbaImage::new(10, 10);
    assert!(
        capture::crop(
            &image,
            PixelRect {
                x: u32::MAX,
                y: 0,
                width: 5,
                height: 5
            }
        )
        .is_none()
    );
    assert!(
        capture::crop(
            &image,
            PixelRect {
                x: 9,
                y: 9,
                width: 5,
                height: 5
            }
        )
        .is_none()
    );
}

#[test]
fn duplicate_capture_and_stale_worker_results_are_rejected() {
    let mut gate = CaptureGate::default();
    let old = gate.begin().expect("first capture");
    assert!(gate.begin().is_none());
    assert!(gate.complete(old));
    let current = gate.begin().expect("new capture");
    assert!(!gate.complete(old));
    assert!(gate.complete(current));
    assert!(!gate.complete(current));
}

#[test]
fn pin_preserves_original_size_and_position_on_a_negative_retina_display() {
    let placement = geometry::pin_placement(
        PixelRect {
            x: 200,
            y: 100,
            width: 1600,
            height: 1200,
        },
        [2880, 1800],
        [-1440.0, -200.0],
        [1440.0, 900.0],
    );
    assert_eq!(placement.position, [-1340.0, -150.0]);
    assert_eq!(placement.size, [800.0, 600.0]);
}
