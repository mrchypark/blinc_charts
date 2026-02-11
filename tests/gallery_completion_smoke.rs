use blinc_charts::prelude::*;
use blinc_core::Point;

#[test]
fn stacked_area_rejects_empty() {
    assert!(StackedAreaChartModel::new(Vec::new()).is_err());
}

#[test]
fn density_map_rejects_empty_points() {
    assert!(DensityMapChartModel::new(Vec::new()).is_err());
}

#[test]
fn contour_rejects_value_length_mismatch() {
    assert!(ContourChartModel::new(4, 3, vec![0.0; 11]).is_err());
    assert!(ContourChartModel::new(4, 3, vec![0.0; 12]).is_ok());
}

#[test]
fn statistics_rejects_empty_groups() {
    assert!(StatisticsChartModel::new(Vec::new()).is_err());
    assert!(StatisticsChartModel::new(vec![vec![1.0, 2.0, 3.0]]).is_ok());
}

#[test]
fn hierarchy_rejects_non_finite_value() {
    let root = HierarchyNode::leaf("root", f32::NAN);
    assert!(HierarchyChartModel::new(root).is_err());
}

#[test]
fn network_rejects_empty_inputs() {
    assert!(NetworkChartModel::new_graph(Vec::new(), Vec::new()).is_err());
    assert!(NetworkChartModel::new_sankey(Vec::new(), Vec::new()).is_err());
    assert!(NetworkChartModel::new_chord(Vec::new(), Vec::new()).is_err());
}

#[test]
fn polar_rejects_empty_dimensions() {
    assert!(PolarChartModel::new_radar(Vec::new(), Vec::new()).is_err());
}

#[test]
fn gauge_clamps_value_and_accepts_range() {
    let m = GaugeChartModel::new(0.0, 100.0, 250.0).unwrap();
    assert_eq!(m.value, 100.0);
}

#[test]
fn geo_rejects_empty_shapes() {
    assert!(GeoChartModel::new(Vec::new()).is_err());
    assert!(GeoChartModel::new(vec![vec![Point::new(0.0, 0.0), Point::new(1.0, 0.0)]]).is_ok());
}
