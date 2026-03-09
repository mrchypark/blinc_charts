mod support;

use criterion::{black_box, criterion_group, criterion_main, BatchSize};

use support::{
    build_lod_cache, build_stitch_seed, build_dense_multi_line_model, criterion_config,
    stitch_visible_edges_like_production, HEIGHT, WIDTH,
};

fn bench_lod_cache_query(c: &mut criterion::Criterion) {
    c.bench_function("lod_query_512_visible_from_64k", |b| {
        let (_series, cache) = build_lod_cache(65_536);
        let mut out = Vec::new();

        b.iter(|| {
            cache.query_into(128.0, 60_000.0, 512, &mut out);
            black_box(out.len());
        });
    });
}

fn bench_lod_cache_stitch_edges(c: &mut criterion::Criterion) {
    c.bench_function("lod_stitch_visible_edges_512_from_64k", |b| {
        let (series, _cache) = build_lod_cache(65_536);
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

fn bench_multi_line_density_hotpath(c: &mut criterion::Criterion) {
    c.bench_function("multi_line_density_hotpath_10k_series", |b| {
        let mut model = build_dense_multi_line_model(10_000, 512);
        model.style.max_series = 1;
        let mut ctx = support::recording_context();

        b.iter(|| {
            model.render_plot(&mut ctx, WIDTH, HEIGHT);
            black_box(ctx.commands().len());
            ctx.clear();
        });
    });
}

criterion_group! {
    name = micro_hotpaths;
    config = criterion_config();
    targets =
        bench_lod_cache_query,
        bench_lod_cache_stitch_edges,
        bench_multi_line_density_hotpath
}
criterion_main!(micro_hotpaths);
