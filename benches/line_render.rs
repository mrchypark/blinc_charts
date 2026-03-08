use std::{sync::Arc, time::Duration};

use blinc_charts::line::LineChartModel;
use blinc_charts::multi_line::MultiLineChartModel;
use blinc_charts::{Domain1D, SeriesLodCache, TimeSeriesF32};
use blinc_core::{Point, RecordingContext, Size};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 720.0;
const PAN_FRAMES_PER_ITER: usize = 10;
const HOVER_SWEEPS_PER_ITER: usize = 10;
const HOVER_STEPS_PER_SWEEP: usize = 100;

fn bench_multi_line_pan(c: &mut Criterion) {
    c.bench_function("multi_line_pan_1k_series_10_frames", |b| {
        let mut model = build_dense_multi_line_model(1_000, 8_192);
        let mut ctx = RecordingContext::new(Size::new(WIDTH, HEIGHT));

        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for frame in 0..PAN_FRAMES_PER_ITER {
                let drag_total = if frame % 2 == 0 { 24.0 } else { -24.0 };
                model.on_drag_pan_total(0.0, WIDTH, HEIGHT);
                model.on_drag_pan_total(drag_total, WIDTH, HEIGHT);
                model.render_plot(&mut ctx, WIDTH, HEIGHT);
                model.on_drag_end();
                ctx.clear();
            }
        });
    });
}

fn bench_line_hover(c: &mut Criterion) {
    c.bench_function("line_hover_1k_moves", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = RecordingContext::new(Size::new(WIDTH, HEIGHT));

        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();
        model.render_overlay(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for sweep in 0..HOVER_SWEEPS_PER_ITER {
                let sweep_offset = sweep as f32 * 0.25;
                for i in 0..HOVER_STEPS_PER_SWEEP {
                    model.on_mouse_move(64.0 + i as f32 * 4.0 + sweep_offset, 120.0, WIDTH, HEIGHT);
                    model.render_overlay(&mut ctx, WIDTH, HEIGHT);
                    ctx.clear();
                }
            }
        });
    });
}

fn bench_multi_line_density_overview(c: &mut Criterion) {
    c.bench_function("multi_line_density_overview_10k_series", |b| {
        let mut model = build_dense_multi_line_model(10_000, 512);
        model.style.max_series = 1;
        let mut ctx = RecordingContext::new(Size::new(WIDTH, HEIGHT));

        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn bench_lod_cache_query(c: &mut Criterion) {
    c.bench_function("lod_cache_query_512_points", |b| {
        let series = build_dense_shared_series(shared_x(65_536), 0);
        let cache = SeriesLodCache::build(&series, 32, 8, 8 * 1024 * 1024);
        let mut out = Vec::new();

        b.iter(|| {
            cache.query_into(128.0, 60_000.0, 512, &mut out);
            black_box(out.len());
        });
    });
}

fn bench_lod_cache_stitch_edges(c: &mut Criterion) {
    c.bench_function("lod_cache_stitch_visible_edges_512_points", |b| {
        let series = build_dense_shared_series(shared_x(65_536), 0);
        let start = series.lower_bound_x(128.0);
        let end = series.upper_bound_x(60_000.0);
        let seed = build_stitch_seed(&series, start, end, 510);

        b.iter_batched(
            || seed.clone(),
            |mut out| {
                stitch_visible_edges_like_production(&series, start, end, &mut out);
                black_box(out.len());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_line_small_window_render(c: &mut Criterion) {
    c.bench_function("line_small_window_raw_visible_11_points", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = RecordingContext::new(Size::new(WIDTH, HEIGHT));
        let windows = [Domain1D::new(100.0, 110.0), Domain1D::new(101.0, 111.0)];
        let mut window_index = 0usize;
        model.view.domain.x = windows[window_index];

        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            window_index ^= 1;
            model.view.domain.x = windows[window_index];
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn build_dense_multi_line_model(
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

fn build_dense_line_model(points: usize) -> LineChartModel {
    let mut x = Vec::with_capacity(points);
    let mut y = Vec::with_capacity(points);

    for i in 0..points {
        let t = i as f32 * 0.001;
        let value = (t * 1.1).sin() * 0.8 + (t * 0.13).sin() * 0.2 + (t * 0.037).cos() * 0.05;
        x.push(i as f32);
        y.push(value);
    }

    LineChartModel::new(TimeSeriesF32::new(x, y).expect("dense line model"))
}

fn build_dense_shared_series(x: Arc<[f32]>, series_index: usize) -> TimeSeriesF32 {
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

fn build_stitch_seed(
    series: &TimeSeriesF32,
    start: usize,
    end: usize,
    target_points: usize,
) -> Vec<Point> {
    let interior_start = start.saturating_add(1);
    let interior_end = end.saturating_sub(1);
    let interior_len = interior_end.saturating_sub(interior_start);
    assert!(
        interior_len > 0,
        "expected stitch benchmark to have interior points"
    );

    let stride = (interior_len / target_points.max(1)).max(1);
    let mut seed = Vec::with_capacity(target_points.min(interior_len));
    let mut idx = interior_start;
    while idx < interior_end && seed.len() < target_points {
        seed.push(series.point(idx));
        idx = idx.saturating_add(stride);
    }
    assert!(!seed.is_empty(), "expected non-empty stitch seed");
    seed
}

// Keep this in sync with `src/lod_cache.rs::stitch_visible_edges` so the
// benchmark can isolate stitching cost without exposing a benchmark-only API.
fn stitch_visible_edges_like_production(
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

fn shared_x(points: usize) -> Arc<[f32]> {
    (0..points).map(|i| i as f32).collect::<Vec<_>>().into()
}

criterion_group! {
    name = line_render;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(8))
        .sample_size(80)
        .noise_threshold(0.05);
    targets =
        bench_multi_line_pan,
        bench_line_hover,
        bench_multi_line_density_overview,
        bench_lod_cache_query,
        bench_lod_cache_stitch_edges,
        bench_line_small_window_render
}
criterion_main!(line_render);
