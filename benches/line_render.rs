use std::sync::Arc;

use blinc_charts::line::LineChartModel;
use blinc_charts::multi_line::MultiLineChartModel;
use blinc_charts::TimeSeriesF32;
use blinc_core::{RecordingContext, Size};
use criterion::{criterion_group, criterion_main, Criterion};

const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 720.0;

fn bench_multi_line_pan(c: &mut Criterion) {
    c.bench_function("multi_line_pan_1k_series", |b| {
        let mut model = build_dense_multi_line_model(1_000, 8_192);
        let mut ctx = RecordingContext::new(Size::new(WIDTH, HEIGHT));
        let mut pan_right = true;

        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            let drag_total = if pan_right { 24.0 } else { -24.0 };
            model.on_drag_pan_total(0.0, WIDTH, HEIGHT);
            model.on_drag_pan_total(drag_total, WIDTH, HEIGHT);
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            model.on_drag_end();
            ctx.clear();
            pan_right = !pan_right;
        });
    });
}

fn bench_line_hover(c: &mut Criterion) {
    c.bench_function("line_hover_100_moves", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = RecordingContext::new(Size::new(WIDTH, HEIGHT));

        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();
        model.render_overlay(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for i in 0..100 {
                model.on_mouse_move(64.0 + i as f32 * 4.0, 120.0, WIDTH, HEIGHT);
                model.render_overlay(&mut ctx, WIDTH, HEIGHT);
                ctx.clear();
            }
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
        let phase = series_index as f32 * 0.013;
        let amplitude = 0.6 + (series_index % 19) as f32 * 0.025;
        let baseline = (series_index % 23) as f32 * 0.03;
        let mut y = Vec::with_capacity(points_per_series);

        for sample_index in 0..points_per_series {
            let t = sample_index as f32 * 0.0025;
            let wave = (t * (1.0 + series_index as f32 * 0.0004) + phase).sin() * amplitude;
            let envelope = (t * 0.17 + phase * 0.5).cos() * 0.22;
            let ripple = ((sample_index % 97) as f32 / 97.0 - 0.5) * 0.05;
            y.push(wave + envelope + ripple + baseline);
        }

        series.push(TimeSeriesF32 {
            x: x.clone(),
            y: y.into(),
        });
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

fn shared_x(points: usize) -> Arc<[f32]> {
    (0..points).map(|i| i as f32).collect::<Vec<_>>().into()
}

criterion_group!(line_render, bench_multi_line_pan, bench_line_hover);
criterion_main!(line_render);
