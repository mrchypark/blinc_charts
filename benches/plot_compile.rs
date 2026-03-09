mod support;

use criterion::{black_box, criterion_group, criterion_main};

use support::{
    build_bar_model_setup, build_line_model_setup, build_multi_line_model_setup,
    build_scatter_model_setup, criterion_config,
};

fn bench_model_build_line(c: &mut criterion::Criterion) {
    c.bench_function("model_build_line_64k", |b| {
        b.iter(|| {
            black_box(build_line_model_setup(65_536));
        });
    });
}

fn bench_model_build_scatter(c: &mut criterion::Criterion) {
    c.bench_function("model_build_scatter_64k", |b| {
        b.iter(|| {
            black_box(build_scatter_model_setup(65_536));
        });
    });
}

fn bench_model_build_bar(c: &mut criterion::Criterion) {
    c.bench_function("model_build_bar_4x8k", |b| {
        b.iter(|| {
            black_box(build_bar_model_setup(4, 8_192));
        });
    });
}

fn bench_model_build_multi_line(c: &mut criterion::Criterion) {
    c.bench_function("model_build_multi_line_1k_series_8k", |b| {
        b.iter(|| {
            black_box(build_multi_line_model_setup(1_000, 8_192));
        });
    });
}

criterion_group! {
    name = plot_compile;
    config = criterion_config();
    targets =
        bench_model_build_line,
        bench_model_build_scatter,
        bench_model_build_bar,
        bench_model_build_multi_line
}
criterion_main!(plot_compile);
