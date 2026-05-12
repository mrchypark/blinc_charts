mod support;

use criterion::{criterion_group, criterion_main};
use std::hint::black_box;

use support::{
    build_dense_area_model, build_dense_bar_model, build_dense_line_model,
    build_dense_multi_line_model, build_dense_scatter_model, build_histogram_model,
    build_small_windows, build_stacked_area_model, build_statistics_model, criterion_config,
    recording_context, HEIGHT, WIDTH,
};

use blinc_charts::statistics::StatisticsMode;

fn bench_line_render_warm(c: &mut criterion::Criterion) {
    c.bench_function("line_render_warm_64k", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        let _ = ctx.take_commands();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_line_small_window_render(c: &mut criterion::Criterion) {
    c.bench_function("line_render_small_window_raw_visible_64k", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = recording_context();
        let windows = build_small_windows();
        let mut index = 0usize;
        model.view.domain.x = windows[index];

        b.iter(|| {
            index ^= 1;
            model.view.domain.x = windows[index];
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_scatter_render_warm(c: &mut criterion::Criterion) {
    c.bench_function("scatter_render_warm_64k", |b| {
        let mut model = build_dense_scatter_model(65_536);
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_bar_render_grouped(c: &mut criterion::Criterion) {
    c.bench_function("bar_render_grouped_4x8k", |b| {
        let mut model = build_dense_bar_model(4, 8_192);
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_area_render_warm(c: &mut criterion::Criterion) {
    c.bench_function("area_render_warm_64k", |b| {
        let mut model = build_dense_area_model(65_536);
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_stacked_area_render(c: &mut criterion::Criterion) {
    c.bench_function("stacked_area_render_32x4k", |b| {
        let mut model = build_stacked_area_model(32, 4_096);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        let _ = ctx.take_commands();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_histogram_render_warm(c: &mut criterion::Criterion) {
    c.bench_function("histogram_render_warm_64k", |b| {
        let mut model = build_histogram_model(65_536);
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_statistics_violin_render(c: &mut criterion::Criterion) {
    c.bench_function("statistics_violin_render_64x128", |b| {
        let model = build_statistics_model(64, 128, StatisticsMode::Violin);
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_statistics_error_band_render(c: &mut criterion::Criterion) {
    c.bench_function("statistics_error_band_render_64x128", |b| {
        let model = build_statistics_model(64, 128, StatisticsMode::ErrorBand);
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_multi_line_render(c: &mut criterion::Criterion) {
    c.bench_function("multi_line_render_1k_series_8k", |b| {
        let mut model = build_dense_multi_line_model(1_000, 8_192);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        let _ = ctx.take_commands();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

fn bench_multi_line_density_overview(c: &mut criterion::Criterion) {
    c.bench_function("multi_line_render_density_overview_10k_series", |b| {
        let mut model = build_dense_multi_line_model(10_000, 512);
        model.style.max_series = 1;
        let mut ctx = recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            let _ = ctx.take_commands();
        });
    });
}

criterion_group! {
    name = cartesian_render;
    config = criterion_config();
    targets =
        bench_line_render_warm,
        bench_line_small_window_render,
        bench_scatter_render_warm,
        bench_bar_render_grouped,
        bench_area_render_warm,
        bench_stacked_area_render,
        bench_histogram_render_warm,
        bench_statistics_violin_render,
        bench_statistics_error_band_render,
        bench_multi_line_render,
        bench_multi_line_density_overview
}
criterion_main!(cartesian_render);
