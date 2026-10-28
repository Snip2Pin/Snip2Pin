use eframe::egui::{Color32, ColorImage};
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
fn odd_retina_edges_expand_to_the_same_native_grid_in_every_drag_direction() {
    let expected = Some(PixelRect {
        x: 200,
        y: 300,
        width: 602,
        height: 402,
    });
    for (start, end) in [
        ([100.5, 150.5], [400.5, 350.5]),
        ([400.5, 350.5], [100.5, 150.5]),
        ([400.5, 150.5], [100.5, 350.5]),
        ([100.5, 350.5], [400.5, 150.5]),
    ] {
        assert_eq!(
            geometry::selection_with_step(start, end, [1000.0, 800.0], [2000, 1600], [2, 2]),
            expected
        );
    }
}

#[test]
fn native_grid_clips_source_edges_and_supports_independent_axis_steps() {
    assert_eq!(
        geometry::selection_with_step(
            [-10.0, 3.5],
            [300.0, 500.0],
            [201.0, 303.0],
            [201, 303],
            [2, 3]
        ),
        Some(PixelRect {
            x: 0,
            y: 3,
            width: 201,
            height: 300
        })
    );
    assert_eq!(
        geometry::selection_with_step(
            [1.0, 1.0],
            [f64::from(u32::MAX); 2],
            [f64::from(u32::MAX); 2],
            [u32::MAX; 2],
            [u32::MAX; 2]
        ),
        Some(PixelRect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX
        })
    );
}

#[test]
fn native_grid_does_not_turn_an_empty_drag_into_a_capture() {
    for (start, end, step) in [
        ([10.5, 20.5], [10.5, 80.0], [2, 2]),
        ([-1.0, 20.5], [-2.0, 80.0], [2, 2]),
        ([10.5, 20.5], [50.0, 80.0], [0, 2]),
    ] {
        assert!(geometry::selection_with_step(start, end, [100.0; 2], [200; 2], step).is_none());
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
fn nonempty_single_pixel_selections_are_allowed() {
    for (start, end, expected) in [
        (
            [10.0, 10.0],
            [11.0, 20.0],
            Some(PixelRect {
                x: 10,
                y: 10,
                width: 1,
                height: 10,
            }),
        ),
        (
            [10.0, 10.0],
            [20.0, 11.0],
            Some(PixelRect {
                x: 10,
                y: 10,
                width: 10,
                height: 1,
            }),
        ),
        (
            [10.0, 10.0],
            [11.0, 11.0],
            Some(PixelRect {
                x: 10,
                y: 10,
                width: 1,
                height: 1,
            }),
        ),
        (
            [10.0, 10.0],
            [12.0, 12.0],
            Some(PixelRect {
                x: 10,
                y: 10,
                width: 2,
                height: 2,
            }),
        ),
        (
            [12.0, 12.0],
            [10.0, 10.0],
            Some(PixelRect {
                x: 10,
                y: 10,
                width: 2,
                height: 2,
            }),
        ),
    ] {
        assert_eq!(
            geometry::selection(start, end, [100.0, 100.0], [100, 100]),
            expected
        );
    }
    assert_eq!(
        geometry::selection([10.0, 10.0], [11.0, 11.0], [100.0, 100.0], [200, 200]),
        Some(PixelRect {
            x: 20,
            y: 20,
            width: 2,
            height: 2
        })
    );
}

#[test]
fn cropped_pixels_are_the_exact_selected_region() {
    let image = ColorImage::new(
        [10, 10],
        (0..10)
            .flat_map(|y| (0..10).map(move |x| Color32::from_rgb(x, y, 0)))
            .collect(),
    );
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
    assert_eq!(cropped.resolution(), [4, 2]);
    assert_eq!(cropped[(0, 0)], Color32::from_rgb(2, 3, 0));
    assert_eq!(cropped[(3, 1)], Color32::from_rgb(5, 4, 0));
}

#[test]
fn malformed_crop_cannot_panic_or_read_outside_image() {
    let image = ColorImage::filled([10, 10], Color32::TRANSPARENT);
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
