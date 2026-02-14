use blinc_charts::axis::build_bottom_ticks;
use blinc_charts::format::{format_compact, format_fixed};
use blinc_charts::interpolate::lerp_f32;
use blinc_charts::polygon::{point_in_polygon, polygon_area, rect_polygon};
use blinc_charts::scale::{BandScale, LinearScale};
use blinc_charts::spatial_index::SpatialIndex;
use blinc_charts::time_format::format_hms;
use blinc_charts::transition::ValueTransition;
use blinc_charts::triangulation::triangulate_fan;
use blinc_charts::Domain1D;
use blinc_core::Point;

#[test]
fn linear_scale_maps_and_inverts() {
    let s = LinearScale::new(0.0, 100.0, 10.0, 210.0);
    assert!((s.map(50.0) - 110.0).abs() < 1e-5);
    assert!((s.invert(110.0) - 50.0).abs() < 1e-5);
}

#[test]
fn band_scale_returns_valid_band_metrics() {
    let b = BandScale::new(4, 0.0, 400.0, 0.1, 0.05);
    assert!(b.band_width() > 0.0);
    assert!(b.center(0).unwrap() < b.center(3).unwrap());
}

#[test]
fn axis_ticks_use_formatter() {
    let ticks = build_bottom_ticks(Domain1D::new(0.0, 100.0), 0.0, 200.0, 5, |v| {
        format!("v={v:.0}")
    });
    assert_eq!(ticks.len(), 5);
    assert!(ticks.iter().all(|t| t.label.starts_with("v=")));
}

#[test]
fn format_helpers_cover_numeric_and_time() {
    assert_eq!(format_fixed(std::f32::consts::PI, 2), "3.14");
    assert_eq!(format_compact(12_400.0), "12.4K");
    assert_eq!(format_hms(3661.0), "01:01:01");
}

#[test]
fn spatial_index_finds_nearest_point() {
    let pts = vec![Point::new(10.0, 10.0), Point::new(50.0, 50.0)];
    let index = SpatialIndex::build(&pts, 8, 8);
    let hit = index.nearest(48.0, 52.0, 10.0).unwrap();
    assert_eq!(hit.0, 1);
}

#[test]
fn triangulation_and_polygon_helpers_work() {
    let square = vec![
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ];
    let tris = triangulate_fan(&square);
    assert!(!tris.is_empty());

    let rect = rect_polygon(0.0, 0.0, 2.0, 1.0);
    assert!(point_in_polygon(Point::new(1.0, 0.5), &rect));
    assert!((polygon_area(&rect) - 2.0).abs() < 1e-5);
}

#[test]
fn interpolate_and_transition_progress_deterministically() {
    assert!((lerp_f32(0.0, 10.0, 0.25) - 2.5).abs() < 1e-6);

    let mut tr = ValueTransition::new(0.0, 100.0, 1.0);
    tr.step(0.5);
    assert!((tr.value() - 50.0).abs() < 1e-3);
    tr.step(0.5);
    assert!((tr.value() - 100.0).abs() < 1e-3);
    assert!(tr.is_finished());
}
