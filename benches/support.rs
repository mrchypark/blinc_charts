#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use blinc_charts::area::AreaChartModel;
use blinc_charts::bar::BarChartModel;
use blinc_charts::gauge::{FunnelChartModel, GaugeChartModel};
use blinc_charts::geo::GeoChartModel;
use blinc_charts::heatmap::HeatmapChartModel;
use blinc_charts::hierarchy::{HierarchyChartModel, HierarchyNode};
use blinc_charts::histogram::HistogramChartModel;
use blinc_charts::line::LineChartModel;
use blinc_charts::multi_line::MultiLineChartModel;
use blinc_charts::network::NetworkChartModel;
use blinc_charts::polar::PolarChartModel;
use blinc_charts::scatter::ScatterChartModel;
use blinc_charts::{Domain1D, SeriesLodCache, TimeSeriesF32};
use blinc_core::{Point, RecordingContext, Size};
use criterion::Criterion;

pub const WIDTH: f32 = 1280.0;
pub const HEIGHT: f32 = 720.0;

pub fn criterion_config() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(6))
        .sample_size(60)
        .noise_threshold(0.05)
}

pub fn recording_context() -> RecordingContext {
    RecordingContext::new(Size::new(WIDTH, HEIGHT))
}

pub fn shared_x(points: usize) -> Arc<[f32]> {
    (0..points).map(|i| i as f32).collect::<Vec<_>>().into()
}

pub fn build_dense_series(points: usize) -> TimeSeriesF32 {
    build_dense_shared_series(shared_x(points), 0)
}

pub fn build_dense_shared_series(x: Arc<[f32]>, series_index: usize) -> TimeSeriesF32 {
    let phase = series_index as f32 * 0.013;
    let amplitude = 0.6 + (series_index % 19) as f32 * 0.025;
    let baseline = (series_index % 23) as f32 * 0.03;
    let mut y = Vec::with_capacity(x.len());

    for sample_index in 0..x.len() {
        let t = sample_index as f32 * 0.0025;
        let wave = (t * (1.0 + series_index as f32 * 0.0004) + phase).sin() * amplitude;
        let envelope = (t * 0.17 + phase * 0.5).cos() * 0.22;
        let ripple = ((sample_index % 97) as f32 / 97.0 - 0.5) * 0.05;
        y.push(wave + envelope + ripple + baseline);
    }

    TimeSeriesF32::from_arcs(x, y.into()).expect("shared dense series")
}

pub fn build_dense_line_model(points: usize) -> LineChartModel {
    LineChartModel::new(build_dense_series(points))
}

pub fn build_dense_scatter_model(points: usize) -> ScatterChartModel {
    ScatterChartModel::new(build_dense_series(points))
}

pub fn build_dense_area_model(points: usize) -> AreaChartModel {
    AreaChartModel::new(build_dense_series(points))
}

pub fn build_dense_bar_model(series_count: usize, points_per_series: usize) -> BarChartModel {
    let x = shared_x(points_per_series);
    let series = (0..series_count)
        .map(|idx| build_dense_shared_series(x.clone(), idx))
        .collect::<Vec<_>>();
    BarChartModel::new(series).expect("dense bar model")
}

pub fn build_histogram_model(values: usize) -> HistogramChartModel {
    let samples = (0..values)
        .map(|i| {
            let t = i as f32 * 0.011;
            (t.sin() * 0.7) + (t * 0.13).cos() * 0.2 + 0.5
        })
        .collect::<Vec<_>>();
    HistogramChartModel::new(samples).expect("histogram model")
}

pub fn build_dense_multi_line_model(
    series_count: usize,
    points_per_series: usize,
) -> MultiLineChartModel {
    let x = shared_x(points_per_series);
    let mut series = Vec::with_capacity(series_count);

    for series_index in 0..series_count {
        series.push(build_dense_shared_series(x.clone(), series_index));
    }

    let mut model = MultiLineChartModel::new(series).expect("dense multi-line model");
    model.style.max_series = series_count;
    model.style.max_points_per_series = 2_048;
    model.style.max_total_segments = 45_000;
    model
}

pub fn build_heatmap_model(grid_w: usize, grid_h: usize) -> HeatmapChartModel {
    let values = (0..grid_h)
        .flat_map(|y| {
            (0..grid_w).map(move |x| {
                let xf = x as f32 / grid_w.max(1) as f32;
                let yf = y as f32 / grid_h.max(1) as f32;
                (xf * 11.0).sin() * 0.6 + (yf * 7.0).cos() * 0.4 + xf * yf
            })
        })
        .collect::<Vec<_>>();
    HeatmapChartModel::new(grid_w, grid_h, values).expect("heatmap model")
}

pub fn build_hierarchy_model(branching: usize, depth: usize) -> HierarchyChartModel {
    fn node(level: usize, branching: usize, depth: usize, index: usize) -> HierarchyNode {
        if level == depth {
            return HierarchyNode::leaf(format!("leaf-{level}-{index}"), 1.0 + index as f32 * 0.01);
        }
        let children = (0..branching)
            .map(|child| node(level + 1, branching, depth, index * branching + child))
            .collect();
        HierarchyNode::node(format!("node-{level}-{index}"), children)
    }

    HierarchyChartModel::new(node(0, branching, depth, 0)).expect("hierarchy model")
}

pub fn build_network_model(nodes: usize, extra_edges: usize) -> NetworkChartModel {
    let labels = (0..nodes).map(|i| format!("N{i}")).collect::<Vec<_>>();
    let mut edges = Vec::with_capacity(nodes + extra_edges);
    for i in 0..nodes.saturating_sub(1) {
        edges.push((i, i + 1));
    }
    for i in 0..extra_edges {
        let a = i % nodes.max(1);
        let b = (i * 7 + 11) % nodes.max(1);
        if a != b {
            edges.push((a, b));
        }
    }
    NetworkChartModel::new_graph(labels, edges).expect("network model")
}

pub fn build_geo_model(shape_count: usize, points_per_shape: usize) -> GeoChartModel {
    let shapes = (0..shape_count)
        .map(|shape| {
            (0..points_per_shape)
                .map(|idx| {
                    let t = idx as f32 / points_per_shape.max(2) as f32;
                    let x = shape as f32 * 0.5 + t * 8.0;
                    let y = ((t * std::f32::consts::TAU * 2.0) + shape as f32 * 0.2).sin() * 2.0;
                    Point::new(x, y)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    GeoChartModel::new(shapes).expect("geo model")
}

pub fn build_gauge_model() -> GaugeChartModel {
    GaugeChartModel::new(0.0, 100.0, 55.0).expect("gauge model")
}

pub fn build_funnel_model(stages: usize) -> FunnelChartModel {
    let data = (0..stages)
        .map(|idx| (format!("Stage {idx}"), (stages - idx) as f32 * 100.0))
        .collect::<Vec<_>>();
    FunnelChartModel::new(data).expect("funnel model")
}

pub fn build_polar_model(dimensions: usize, series_count: usize) -> PolarChartModel {
    let dims = (0..dimensions).map(|idx| format!("D{idx}")).collect::<Vec<_>>();
    let series = (0..series_count)
        .map(|series_idx| {
            (0..dimensions)
                .map(|dim| {
                    let t = dim as f32 / dimensions.max(1) as f32;
                    ((t * 9.0 + series_idx as f32 * 0.3).sin() * 0.4 + 0.5).clamp(0.0, 1.0)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    PolarChartModel::new_radar(dims, series).expect("polar model")
}

pub fn build_line_model_setup(count: usize) -> LineChartModel {
    LineChartModel::new(build_dense_series(count))
}

pub fn build_scatter_model_setup(count: usize) -> ScatterChartModel {
    ScatterChartModel::new(build_dense_series(count))
}

pub fn build_bar_model_setup(series_count: usize, points_per_series: usize) -> BarChartModel {
    build_dense_bar_model(series_count, points_per_series)
}

pub fn build_multi_line_model_setup(
    series_count: usize,
    points_per_series: usize,
) -> MultiLineChartModel {
    build_dense_multi_line_model(series_count, points_per_series)
}

pub fn build_small_windows() -> [Domain1D; 2] {
    [Domain1D::new(100.0, 110.0), Domain1D::new(101.0, 111.0)]
}

pub fn build_lod_cache(points: usize) -> (TimeSeriesF32, SeriesLodCache) {
    let series = build_dense_series(points);
    let cache = SeriesLodCache::build(&series, 32, 8, 8 * 1024 * 1024);
    (series, cache)
}

pub fn build_stitch_seed(
    series: &TimeSeriesF32,
    start: usize,
    end: usize,
    target_points: usize,
) -> Vec<Point> {
    let interior_start = start.saturating_add(1);
    let interior_end = end.saturating_sub(1);
    let interior_len = interior_end.saturating_sub(interior_start);
    assert!(interior_len > 0, "expected interior points");

    let stride = (interior_len / target_points.max(1)).max(1);
    let mut seed = Vec::with_capacity(target_points.min(interior_len));
    let mut idx = interior_start;
    while idx < interior_end && seed.len() < target_points {
        seed.push(series.point(idx));
        idx = idx.saturating_add(stride);
    }
    seed
}

pub fn stitch_visible_edges_like_production(
    series: &TimeSeriesF32,
    start: usize,
    end: usize,
    out: &mut Vec<Point>,
) {
    if start >= end || end > series.len() {
        out.clear();
        return;
    }

    let first = series.point(start);
    let last = series.point(end - 1);

    if out.is_empty() {
        out.push(first);
        if last != first {
            out.push(last);
        }
        return;
    }

    let prepend_first = out.first().is_some_and(|p| *p != first);
    let append_last = out.last().is_some_and(|p| *p != last);

    if prepend_first {
        let mut stitched = Vec::with_capacity(out.len() + 1 + usize::from(append_last));
        stitched.push(first);
        stitched.extend_from_slice(out);
        if append_last {
            stitched.push(last);
        }
        stitched.dedup_by(|a, b| a.x == b.x && a.y == b.y);
        *out = stitched;
        return;
    }

    if append_last {
        out.push(last);
    }
    out.dedup_by(|a, b| a.x == b.x && a.y == b.y);
}
