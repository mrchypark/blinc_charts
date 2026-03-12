mod support;

use criterion::{criterion_group, criterion_main};
use std::hint::black_box;

use support::{
    build_funnel_model, build_gauge_model, build_geo_model, build_heatmap_model,
    build_hierarchy_model, build_network_model, build_polar_model, criterion_config,
    recording_context, HEIGHT, WIDTH,
};

fn bench_heatmap_render(c: &mut criterion::Criterion) {
    c.bench_function("heatmap_render_256x128", |b| {
        let model = build_heatmap_model(256, 128);
        let mut ctx = recording_context();
        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn bench_hierarchy_render(c: &mut criterion::Criterion) {
    c.bench_function("hierarchy_render_branch4_depth6", |b| {
        let mut model = build_hierarchy_model(4, 6);
        let mut ctx = recording_context();
        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn bench_network_hover_and_render(c: &mut criterion::Criterion) {
    c.bench_function("network_hover_plot_1k_nodes_5k_edges", |b| {
        let mut model = build_network_model(1_000, 5_000);
        let mut ctx = recording_context();
        b.iter(|| {
            model.on_mouse_move(320.0, 220.0, WIDTH, HEIGHT);
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn bench_geo_pan_and_render(c: &mut criterion::Criterion) {
    c.bench_function("geo_pan_plot_100_shapes_1k_points", |b| {
        let mut model = build_geo_model(100, 1_000);
        let mut ctx = recording_context();
        b.iter(|| {
            model.on_drag_pan_total(16.0, -8.0, WIDTH, HEIGHT);
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            model.on_drag_end();
            ctx.clear();
        });
    });
}

fn bench_gauge_render(c: &mut criterion::Criterion) {
    c.bench_function("gauge_render_single", |b| {
        let model = build_gauge_model();
        let mut ctx = recording_context();
        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn bench_funnel_render(c: &mut criterion::Criterion) {
    c.bench_function("funnel_render_8_stages", |b| {
        let model = build_funnel_model(8);
        let mut ctx = recording_context();
        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

fn bench_polar_render(c: &mut criterion::Criterion) {
    c.bench_function("polar_render_32_dims_16_series", |b| {
        let model = build_polar_model(32, 16);
        let mut ctx = recording_context();
        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

criterion_group! {
    name = family_render;
    config = criterion_config();
    targets =
        bench_heatmap_render,
        bench_hierarchy_render,
        bench_network_hover_and_render,
        bench_geo_pan_and_render,
        bench_gauge_render,
        bench_funnel_render,
        bench_polar_render
}
criterion_main!(family_render);
