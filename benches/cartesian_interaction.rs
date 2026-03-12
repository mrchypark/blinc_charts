mod support;

use criterion::{criterion_group, criterion_main};
use std::hint::black_box;

use support::{
    build_dense_line_model, build_dense_multi_line_model, build_dense_scatter_model,
    criterion_config, recording_context, HEIGHT, WIDTH,
};

const HOVER_STEPS: usize = 1_000;
const PAN_FRAMES: usize = 10;
const ZOOM_STEPS: usize = 100;

fn bench_line_hover_handler(c: &mut criterion::Criterion) {
    c.bench_function("line_hover_handler_1k_moves_64k", |b| {
        let mut model = build_dense_line_model(65_536);

        b.iter(|| {
            for i in 0..HOVER_STEPS {
                let x = 64.0 + i as f32 * 1.2;
                let damage = model.on_mouse_move(x, 120.0, WIDTH, HEIGHT);
                black_box(damage);
            }
        });
    });
}

fn bench_line_hover_overlay(c: &mut criterion::Criterion) {
    c.bench_function("line_hover_overlay_1k_moves_64k", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for i in 0..HOVER_STEPS {
                let x = 64.0 + i as f32 * 1.2;
                model.on_mouse_move(x, 120.0, WIDTH, HEIGHT);
                model.render_overlay(&mut ctx, WIDTH, HEIGHT);
                black_box(ctx.commands().len());
                ctx.clear();
            }
        });
    });
}

fn bench_scatter_hover_overlay(c: &mut criterion::Criterion) {
    c.bench_function("scatter_hover_overlay_1k_moves_64k", |b| {
        let mut model = build_dense_scatter_model(65_536);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for i in 0..HOVER_STEPS {
                let x = 80.0 + i as f32 * 1.1;
                let y = 160.0 + (i % 5) as f32;
                model.on_mouse_move(x, y, WIDTH, HEIGHT);
                model.render_overlay(&mut ctx, WIDTH, HEIGHT);
                black_box(ctx.commands().len());
                ctx.clear();
            }
        });
    });
}

fn bench_line_pan_plot(c: &mut criterion::Criterion) {
    c.bench_function("line_pan_plot_10_frames_64k", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for frame in 0..PAN_FRAMES {
                let drag_total = if frame % 2 == 0 { 24.0 } else { -24.0 };
                model.on_drag_pan_total(0.0, WIDTH, HEIGHT);
                model.on_drag_pan_total(drag_total, WIDTH, HEIGHT);
                model.render_plot(&mut ctx, WIDTH, HEIGHT);
                black_box(ctx.commands().len());
                model.on_drag_end();
                ctx.clear();
            }
        });
    });
}

fn bench_line_scroll_plot(c: &mut criterion::Criterion) {
    c.bench_function("line_scroll_plot_100_steps_64k", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = recording_context();

        b.iter(|| {
            for i in 0..ZOOM_STEPS {
                let cursor = 120.0 + (i % 200) as f32 * 4.0;
                model.on_scroll(-32.0, cursor, WIDTH, HEIGHT);
                model.render_plot(&mut ctx, WIDTH, HEIGHT);
                black_box(ctx.commands().len());
                ctx.clear();
            }
        });
    });
}

fn bench_line_pinch_plot(c: &mut criterion::Criterion) {
    c.bench_function("line_pinch_plot_100_steps_64k", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = recording_context();

        b.iter(|| {
            for i in 0..ZOOM_STEPS {
                let cursor = 180.0 + (i % 160) as f32 * 3.0;
                model.on_pinch(1.01, cursor, WIDTH, HEIGHT);
                model.render_plot(&mut ctx, WIDTH, HEIGHT);
                black_box(ctx.commands().len());
                ctx.clear();
            }
        });
    });
}

fn bench_multi_line_pan_plot(c: &mut criterion::Criterion) {
    c.bench_function("multi_line_pan_plot_10_frames_1k_series_8k", |b| {
        let mut model = build_dense_multi_line_model(1_000, 8_192);
        let mut ctx = recording_context();
        model.render_plot(&mut ctx, WIDTH, HEIGHT);
        ctx.clear();

        b.iter(|| {
            for frame in 0..PAN_FRAMES {
                let drag_total = if frame % 2 == 0 { 24.0 } else { -24.0 };
                model.on_drag_pan_total(0.0, WIDTH, HEIGHT);
                model.on_drag_pan_total(drag_total, WIDTH, HEIGHT);
                model.render_plot(&mut ctx, WIDTH, HEIGHT);
                black_box(ctx.commands().len());
                model.on_drag_end();
                ctx.clear();
            }
        });
    });
}

criterion_group! {
    name = cartesian_interaction;
    config = criterion_config();
    targets =
        bench_line_hover_handler,
        bench_line_hover_overlay,
        bench_scatter_hover_overlay,
        bench_line_pan_plot,
        bench_line_scroll_plot,
        bench_line_pinch_plot,
        bench_multi_line_pan_plot
}
criterion_main!(cartesian_interaction);
