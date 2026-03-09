# Benchmarks

`blinc_charts` uses a lab-style benchmark suite. The goal is broad observability first:
render cost, interaction latency, compile/setup cost, and pure hot-path cost are measured
separately so regressions are easier to localize.

## Bench Layout

- `cartesian_render`
  - steady-state `render_plot` cost for `line`, `scatter`, `bar`, `area`, `histogram`, `multi_line`
  - narrow-window raw-visible fallback
  - density-overview fallback for very large `multi_line`
- `cartesian_interaction`
  - `hover`, `hover + overlay redraw`, `pan + redraw`, `scroll zoom + redraw`, `pinch + redraw`
  - interaction paths are measured directly on `*ChartModel` methods instead of benchmarking the full UI event shell
- `family_render`
  - `heatmap`, `hierarchy`, `network`, `geo`, `gauge`, `funnel`, `polar`
- `plot_compile`
  - constructor and setup cost for large chart models
  - line, scatter, bar, and multi-line setup paths
- `micro_hotpaths`
  - `SeriesLodCache::query_into`
  - visible-edge stitching
  - density-heavy render path

## How To Run

Check all bench targets compile:

```bash
cargo check --benches
```

Run a single bench file:

```bash
cargo bench --bench cartesian_render -- --noplot
```

Run a single benchmark within a file:

```bash
cargo bench --bench plot_compile model_build_line_64k -- --sample-size 10
```

Capture a named baseline:

```bash
cargo bench --bench cartesian_render -- --noplot --save-baseline main
```

Compare against a named baseline:

```bash
cargo bench --bench cartesian_render -- --noplot --baseline main
```

If you need fresh absolute numbers without old Criterion output:

```bash
rm -rf target/criterion
```

That also removes named baselines, so recapture them before using `--baseline`.

## Interpretation

Use absolute budgets as the primary gate.
Use `% change` from a named baseline as secondary context.

For interaction benchmarks:

- `handler` benchmarks represent event-processing latency only
- `overlay` or `plot` benchmarks represent user-visible event + redraw latency

For render benchmarks:

- treat timings as frame cost
- `ctx.commands().len()` is black-boxed so the work cannot be optimized away

For setup benchmarks:

- treat timings as chart setup cost per build
- compare model families separately

For microbenchmarks:

- use them to explain regressions, not as the sole UX proxy

## Budget Bands

The numbers below are working budgets for development on the primary machine at `1280x720`.
They are not all CI fail gates. Read them as:

- `Goal`: healthy target
- `Warning`: investigate if persistent
- `Fail candidate`: likely regression

### Plot Compile

- `model_build_line_64k`
  - Goal: `< 40 ms`
  - Warning: `40-60 ms`
  - Fail candidate: `> 60 ms`
- `model_build_scatter_64k`
  - Goal: `< 40 ms`
  - Warning: `40-60 ms`
  - Fail candidate: `> 60 ms`
- `model_build_bar_4x8k`
  - Goal: `< 30 ms`
  - Warning: `30-50 ms`
  - Fail candidate: `> 50 ms`
- `model_build_multi_line_1k_series_8k`
  - Goal: `< 250 ms`
  - Warning: `250-400 ms`
  - Fail candidate: `> 400 ms`

### Cartesian Render

- `line_render_warm_64k`
  - Goal: `< 4 ms`
  - Warning: `4-8 ms`
  - Fail candidate: `> 8 ms`
- `scatter_render_warm_64k`
  - Goal: `< 6 ms`
  - Warning: `6-10 ms`
  - Fail candidate: `> 10 ms`
- `area_render_warm_64k`
  - Goal: `< 5 ms`
  - Warning: `5-9 ms`
  - Fail candidate: `> 9 ms`
- `histogram_render_warm_64k`
  - Goal: `< 3 ms`
  - Warning: `3-6 ms`
  - Fail candidate: `> 6 ms`
- `bar_render_grouped_4x8k`
  - Goal: `< 6 ms`
  - Warning: `6-10 ms`
  - Fail candidate: `> 10 ms`
- `multi_line_render_1k_series_8k`
  - Goal: `< 12 ms`
  - Warning: `12-20 ms`
  - Fail candidate: `> 20 ms`
- `multi_line_render_density_overview_10k_series`
  - Goal: `< 8 ms`
  - Warning: `8-14 ms`
  - Fail candidate: `> 14 ms`

### Cartesian Interaction

- `line_hover_handler_1k_moves_64k`
  - Goal: `< 0.03 ms/event`
  - Warning: `0.03-0.08 ms/event`
  - Fail candidate: `> 0.08 ms/event`
- `line_hover_overlay_1k_moves_64k`
  - Goal: `< 0.20 ms/event`
  - Warning: `0.20-0.50 ms/event`
  - Fail candidate: `> 0.50 ms/event`
- `scatter_hover_overlay_1k_moves_64k`
  - Goal: `< 0.35 ms/event`
  - Warning: `0.35-0.80 ms/event`
  - Fail candidate: `> 0.80 ms/event`
- `line_pan_plot_10_frames_64k`
  - Goal: `< 6 ms/frame`
  - Warning: `6-12 ms/frame`
  - Fail candidate: `> 12 ms/frame`
- `line_scroll_plot_100_steps_64k`
  - Goal: `< 4 ms/step`
  - Warning: `4-8 ms/step`
  - Fail candidate: `> 8 ms/step`
- `line_pinch_plot_100_steps_64k`
  - Goal: `< 4 ms/step`
  - Warning: `4-8 ms/step`
  - Fail candidate: `> 8 ms/step`
- `multi_line_pan_plot_10_frames_1k_series_8k`
  - Goal: `< 14 ms/frame`
  - Warning: `14-22 ms/frame`
  - Fail candidate: `> 22 ms/frame`

### Family Render

- `heatmap_render_256x128`
  - Goal: `< 8 ms`
  - Warning: `8-14 ms`
  - Fail candidate: `> 14 ms`
- `hierarchy_render_branch4_depth6`
  - Goal: `< 10 ms`
  - Warning: `10-18 ms`
  - Fail candidate: `> 18 ms`
- `network_hover_plot_1k_nodes_5k_edges`
  - Goal: `< 12 ms`
  - Warning: `12-20 ms`
  - Fail candidate: `> 20 ms`
- `geo_pan_plot_100_shapes_1k_points`
  - Goal: `< 12 ms`
  - Warning: `12-20 ms`
  - Fail candidate: `> 20 ms`
- `gauge_render_single`
  - Goal: `< 1 ms`
  - Warning: `1-2 ms`
  - Fail candidate: `> 2 ms`
- `funnel_render_8_stages`
  - Goal: `< 2 ms`
  - Warning: `2-4 ms`
  - Fail candidate: `> 4 ms`
- `polar_render_32_dims_16_series`
  - Goal: `< 6 ms`
  - Warning: `6-10 ms`
  - Fail candidate: `> 10 ms`

### Micro Hot Paths

- `lod_query_512_visible_from_64k`
  - Goal: `< 0.15 ms`
  - Warning: `0.15-0.35 ms`
  - Fail candidate: `> 0.35 ms`
- `lod_stitch_visible_edges_512_from_64k`
  - Goal: `< 0.05 ms`
  - Warning: `0.05-0.12 ms`
  - Fail candidate: `> 0.12 ms`

## Practical Policy

- Do not treat a single noisy run as a regression.
- Treat absolute budget violations as stronger signals than percentage change.
- Investigate persistent regressions above `25%` even when still inside the warning band.
- For future CI gating, start with a small subset:
  - `model_build_line_64k`
  - `line_render_warm_64k`
  - `line_hover_overlay_1k_moves_64k`
  - `multi_line_pan_plot_10_frames_1k_series_8k`
  - `lod_query_512_visible_from_64k`

## CI Gate

CI uses the five benchmarks above as the initial hard gate.

- Absolute failure uses the `Fail candidate` budget.
- Relative failure uses `> 25%` regression versus the latest uploaded `main` branch baseline artifact.
- If no baseline artifact is available yet, CI falls back to absolute budgets only.

The CI gate configuration lives in:

- [benchmarks/ci_benchmarks.json](/Users/cypark/.codex/worktrees/9cb1/blinc_charts/benchmarks/ci_benchmarks.json)
- [scripts/bench_ci.py](/Users/cypark/.codex/worktrees/9cb1/blinc_charts/scripts/bench_ci.py)
- [.github/workflows/ci.yml](/Users/cypark/.codex/worktrees/9cb1/blinc_charts/.github/workflows/ci.yml)
