# Line Rendering Performance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce frame time and redraw cost for dense line and multi-line charts, especially when many series are visible and hover, pan, and zoom events arrive at high frequency.

**Architecture:** Start by measuring the current behavior and locking in absolute budgets. Then land local `blinc_charts` changes that remove avoidable CPU work, cap memory growth, and provide a safe fallback for pathological series counts. Only if those local changes still miss the budget should we do cross-repo work in `Blinc` for a batched multi-polyline API.

**Tech Stack:** Rust 2021, `blinc_charts`, `blinc_core`, `blinc_gpu`, `blinc_layout`, `RecordingContext`, `cargo test`, `cargo bench`

---

**Performance Targets**

- `multi_line_pan_1k_series` median benchmark time: `< 8 ms` at `1280x720` after warm cache on the primary dev machine
- `line_hover_100_moves` median benchmark time: `< 2 ms`
- No local phase should increase `RecordingContext` command count for the same rendered geometry
- LOD cache memory in `MultiLineChartModel` must stay under a documented byte budget; default target: `<= 64 MiB`

### Task 0: Capture baseline and define budgets

**Files:**
- Modify: `Cargo.toml`
- Create: `benches/line_render.rs`
- Modify: `README.md`

**Step 1: Write the benchmark harness**

```rust
fn bench_multi_line_pan(c: &mut Criterion) {
    c.bench_function("multi_line_pan_1k_series", |b| {
        let mut model = build_dense_multi_line_model(1_000, 8_192);
        let mut ctx = RecordingContext::new(Size::new(1280.0, 720.0));
        b.iter(|| {
            model.on_scroll(24.0, 640.0, 1280.0, 720.0);
            model.render_plot(&mut ctx, 1280.0, 720.0);
            ctx.clear();
        });
    });
}

fn bench_line_hover(c: &mut Criterion) {
    c.bench_function("line_hover_100_moves", |b| {
        let mut model = build_dense_line_model(65_536);
        let mut ctx = RecordingContext::new(Size::new(1280.0, 720.0));
        b.iter(|| {
            for i in 0..100 {
                model.on_mouse_move(64.0 + i as f32 * 4.0, 120.0, 1280.0, 720.0);
                model.render_overlay(&mut ctx, 1280.0, 720.0);
                ctx.clear();
            }
        });
    });
}
```

**Step 2: Run the benchmark to capture the baseline**

Run: `cargo bench --bench line_render -- --noplot`
Expected: Benchmark completes and records the first baseline that all later tasks compare against.

**Step 3: Wire the harness and document the budgets**

```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "line_render"
harness = false
```

Document the absolute targets above in `README.md`. Do not use a relative target like "30% faster" as the primary gate; keep percentage improvement as secondary context only.

**Step 4: Run the benchmark again to verify the harness is stable**

Run: `cargo bench --bench line_render -- --noplot`
Expected: Benchmark completes twice with similar ordering and no harness errors.

**Step 5: Commit**

```bash
git add Cargo.toml benches/line_render.rs README.md
git commit -m "bench: add baseline line rendering benchmarks"
```

### Task 1: Add redraw-intent scaffolding and no-op redraw tests

**Files:**
- Modify: `src/xy_stack.rs`
- Modify: `src/line.rs`
- Modify: `src/multi_line.rs`
- Modify: `src/scatter.rs`

**Step 1: Write the failing unit tests**

```rust
use crate::TimeSeriesF32;
use crate::xy_stack::ChartDamage;

#[test]
fn line_mouse_move_inside_plot_returns_overlay_damage() {
    let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
    let mut model = LineChartModel::new(series);
    let damage = model.on_mouse_move(120.0, 40.0, 320.0, 200.0);
    assert_eq!(damage, ChartDamage::Overlay);
}

#[test]
fn repeated_outside_mouse_move_returns_no_damage() {
    let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
    let mut model = ScatterChartModel::new(series);
    assert_eq!(model.on_mouse_move(-10.0, -10.0, 320.0, 200.0), ChartDamage::None);
    assert_eq!(model.on_mouse_move(-10.0, -10.0, 320.0, 200.0), ChartDamage::None);
}

#[test]
fn multi_line_scroll_returns_plot_damage() {
    let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
    let mut model = MultiLineChartModel::new(vec![series]).unwrap();
    let damage = model.on_scroll(40.0, 120.0, 320.0, 200.0);
    assert_eq!(damage, ChartDamage::Plot);
}
```

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib`
Expected: FAIL because handlers still return `()`, and `ChartDamage` does not exist.

**Step 3: Write the minimal implementation**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChartDamage {
    None,
    Overlay,
    Plot,
}

impl ChartDamage {
    pub(crate) fn needs_redraw(self) -> bool {
        !matches!(self, Self::None)
    }
}
```

Update `InteractiveXChartModel` so input handlers return `ChartDamage`, and update `x_chart` / `linked_x_chart` to call `request_redraw()` only when `damage.needs_redraw()` is true.

Important:
- This task does **not** fully solve plot/overlay invalidation under the current `blinc_layout` redraw model.
- The purpose of this task is to remove redundant redraws, make intent explicit, and create a hook for later retained-plot work if the engine grows that capability.

**Step 4: Run the tests to verify they pass**

Run: `cargo test --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/xy_stack.rs src/line.rs src/multi_line.rs src/scatter.rs
git commit -m "perf: add redraw intent for interactive charts"
```

### Task 2: Replace per-point scale construction with a reusable affine plot transform

**Files:**
- Modify: `src/view.rs`
- Modify: `src/line.rs`
- Modify: `src/multi_line.rs`
- Modify: `src/scatter.rs`
- Test: `src/view.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn plot_affine_matches_existing_point_mapping() {
    let view = ChartView::new(Domain2D::new(Domain1D::new(10.0, 20.0), Domain1D::new(-5.0, 5.0)));
    let affine = view.plot_affine(32.0, 16.0, 200.0, 100.0);
    let p = Point::new(15.0, 1.5);
    assert_eq!(affine.map_point(p), view.data_to_px(p, 32.0, 16.0, 200.0, 100.0));
}
```

**Step 2: Run the test to verify it fails**

Run: `cargo test plot_affine_matches_existing_point_mapping --lib`
Expected: FAIL because `plot_affine()` does not exist.

**Step 3: Write the minimal implementation**

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotAffine {
    x_scale: f64,
    x_bias: f64,
    y_scale: f64,
    y_bias: f64,
}

impl PlotAffine {
    pub fn map_point(self, p: Point) -> Point {
        Point::new(
            (self.x_scale * p.x as f64 + self.x_bias) as f32,
            (self.y_scale * p.y as f64 + self.y_bias) as f32,
        )
    }
}
```

Compute the coefficients once per frame in `ChartView::plot_affine()`, then use `affine.map_point(*p)` inside the hot loops in `line.rs`, `multi_line.rs`, and `scatter.rs`.

**Step 4: Run the test to verify it passes**

Run: `cargo test plot_affine_matches_existing_point_mapping --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/view.rs src/line.rs src/multi_line.rs src/scatter.rs
git commit -m "perf: use affine plot mapping in chart hot loops"
```

### Task 3: Land the cheap local wins: infinite-gap fast path and axis cache

**Files:**
- Modify: `src/segments.rs`
- Modify: `src/multi_line.rs`
- Modify: `src/line.rs`
- Modify: `src/axis.rs`
- Test: `src/segments.rs`
- Test: `src/line.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn runs_with_infinite_gap_emit_single_run() {
    let pts = [Point::new(0.0, 0.0), Point::new(1.0, 1.0), Point::new(10.0, 2.0)];
    let mut out = Vec::new();
    runs_by_gap(&pts, f32::INFINITY, &mut out);
    assert_eq!(out, vec![(0, 3)]);
}

#[test]
fn line_axis_cache_key_changes_when_domain_changes() {
    let a = LineAxisCacheKey::new(Domain1D::new(0.0, 10.0), Domain1D::new(-1.0, 1.0), 32.0, 16.0, 200.0, 100.0);
    let b = LineAxisCacheKey::new(Domain1D::new(1.0, 10.0), Domain1D::new(-1.0, 1.0), 32.0, 16.0, 200.0, 100.0);
    assert_ne!(a, b);
}
```

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib`
Expected: FAIL because the fast path and `LineAxisCacheKey` do not exist.

**Step 3: Write the minimal implementation**

```rust
pub fn runs_by_gap(points: &[Point], gap_dx: f32, out: &mut Vec<(usize, usize)>) {
    out.clear();
    if points.is_empty() {
        return;
    }
    if gap_dx.is_infinite() {
        out.push((0, points.len()));
        return;
    }
    // existing scan follows
}
```

Add a small axis cache in `line.rs` keyed by domain bits plus plot geometry so hover-only overlay redraws reuse the last tick vectors and formatted labels. Keep the cache local to `LineChartModel`; do not generalize it in this task.

**Step 4: Run the tests to verify they pass**

Run: `cargo test --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/segments.rs src/multi_line.rs src/line.rs src/axis.rs
git commit -m "perf: add gap fast path and cache line axis ticks"
```

### Task 4: Add a multiresolution min/max LOD cache with memory bounds and quality checks

**Files:**
- Create: `src/lod_cache.rs`
- Modify: `src/lib.rs`
- Modify: `src/line.rs`
- Modify: `src/multi_line.rs`
- Test: `src/lod_cache.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn lod_cache_query_is_ordered_and_bounded() {
    let x: Vec<f32> = (0..10_000).map(|i| i as f32).collect();
    let y: Vec<f32> = (0..10_000).map(|i| (i as f32).sin()).collect();
    let series = TimeSeriesF32::new(x, y).unwrap();
    let cache = SeriesLodCache::build(&series, 32, 8, 1 << 20);
    let mut out = Vec::new();
    cache.query_into(100.0, 9000.0, 512, &mut out);
    assert!(out.len() <= 520);
    assert!(out.windows(2).all(|w| w[0].x <= w[1].x));
}

#[test]
fn lod_cache_respects_byte_budget() {
    let x: Vec<f32> = (0..50_000).map(|i| i as f32).collect();
    let y: Vec<f32> = (0..50_000).map(|i| (i as f32 * 0.01).sin()).collect();
    let series = TimeSeriesF32::new(x, y).unwrap();
    let cache = SeriesLodCache::build(&series, 32, 8, 1 << 20);
    assert!(cache.approx_bytes() <= 1 << 20);
}

#[test]
fn lod_cache_preserves_bucket_extrema_envelope() {
    let x: Vec<f32> = (0..4096).map(|i| i as f32).collect();
    let mut y = vec![0.0; 4096];
    y[1024] = 100.0;
    y[2048] = -100.0;
    let series = TimeSeriesF32::new(x, y).unwrap();
    let cache = SeriesLodCache::build(&series, 32, 8, 1 << 20);
    let mut out = Vec::new();
    cache.query_into(0.0, 4095.0, 128, &mut out);
    assert!(out.iter().any(|p| p.y >= 100.0));
    assert!(out.iter().any(|p| p.y <= -100.0));
}
```

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib`
Expected: FAIL because `SeriesLodCache` does not exist.

**Step 3: Write the minimal implementation**

```rust
pub struct SeriesLodCache {
    levels: Vec<Vec<Point>>,
    approx_bytes: usize,
}

impl SeriesLodCache {
    pub fn build(series: &TimeSeriesF32, min_bucket: usize, max_levels: usize, max_bytes: usize) -> Self { /* build bounded min/max pyramid */ }
    pub fn query_into(&self, x_min: f32, x_max: f32, max_points: usize, out: &mut Vec<Point>) { /* choose best level without allocating */ }
    pub fn approx_bytes(&self) -> usize { self.approx_bytes }
}
```

Implementation rules:
- Use `query_into(..., &mut Vec<Point>)`, not `-> Vec<Point>`, to avoid per-frame allocations
- Build levels lazily or stop early once the byte budget is hit
- Choose the best level by pixel-to-point ratio, not ad hoc
- If the cache cannot satisfy the request inside the byte budget, fall back to `downsample_min_max()`

**Step 4: Run the tests to verify they pass**

Run: `cargo test --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lod_cache.rs src/lib.rs src/line.rs src/multi_line.rs
git commit -m "perf: add bounded multiresolution lod cache for line charts"
```

### Task 5: Add automatic density fallback for pathological series counts

**Files:**
- Modify: `src/multi_line.rs`
- Modify: `src/density_map.rs`
- Modify: `src/lib.rs`
- Test: `src/multi_line.rs`

**Step 1: Write the failing test**

```rust
use crate::multi_line::MultiLineChartModel;
use crate::TimeSeriesF32;

#[test]
fn multi_line_switches_to_density_mode_when_series_budget_is_exceeded() {
    let series: Vec<TimeSeriesF32> = (0..10_000)
        .map(|_| TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 0.0]).unwrap())
        .collect();
    let model = MultiLineChartModel::new(series).unwrap();
    assert!(model.should_use_density_overview(1280.0));
}
```

**Step 2: Run the test to verify it fails**

Run: `cargo test --lib`
Expected: FAIL because the fallback heuristic does not exist.

**Step 3: Write the minimal implementation**

```rust
fn should_use_density_overview(&self, plot_w: f32) -> bool {
    self.series.len() > self.style.max_series
        || self.style.max_total_segments > (plot_w as usize * 16)
}
```

When the heuristic trips, render a density-style overview instead of issuing thousands of line runs. Keep the existing public API; this is an internal rendering choice.

**Step 4: Run the test and benchmark to verify it passes**

Run: `cargo test --lib`
Expected: PASS

Run: `cargo bench --bench line_render -- --noplot`
Expected: The pathological case does not regress the dense multi-line benchmark.

**Step 5: Commit**

```bash
git add src/multi_line.rs src/density_map.rs src/lib.rs
git commit -m "perf: add density fallback for pathological multi-line cases"
```

### Task 6: If local changes still miss budget, add an optional upstream batched multi-polyline path

**Files:**
- Modify in upstream `Blinc` repo: `crates/blinc_core/src/draw.rs`
- Modify in upstream `Blinc` repo: `crates/blinc_gpu/src/paint.rs`
- Modify in upstream `Blinc` repo: `crates/blinc_app/src/app.rs`
- Modify: `src/multi_line.rs`
- Modify: `Cargo.toml`

**Step 1: Write the failing upstream API test**

```rust
#[test]
fn recording_context_can_record_multiple_polylines_in_one_call() {
    let mut ctx = RecordingContext::new(Size::new(320.0, 200.0));
    let polylines = [&[Point::new(0.0, 0.0), Point::new(10.0, 10.0)][..]];
    ctx.stroke_multi_polyline(&polylines, &Stroke::new(1.0), Brush::Solid(Color::WHITE));
    assert_eq!(ctx.commands().len(), 1);
}
```

**Step 2: Run the upstream test to verify it fails**

Run in `Blinc` worktree: `cargo test stroke_multi_polyline --lib`
Expected: FAIL because `stroke_multi_polyline()` does not exist.

**Step 3: Write the minimal implementation**

```rust
fn stroke_multi_polyline(
    &mut self,
    polylines: &[&[Point]],
    stroke: &Stroke,
    brush: Brush,
) {
    for polyline in polylines {
        self.stroke_polyline(polyline, stroke, brush);
    }
}
```

Then optimize the GPU backend so solid-color polylines push all segments through one batch-facing API without rebuilding clip or stroke state per run.

Contingency:
- If the current session cannot edit the upstream `Blinc` worktree, stop after Task 5 and file a follow-up task instead of blocking this plan
- If upstream rejects the API, keep the local density fallback from Task 5 as the terminal mitigation and document the upstream dependency in `README.md`

**Step 4: Run upstream tests, this crate's tests, and the benchmark**

Run in `Blinc` worktree: `cargo test --lib`
Expected: PASS

Run here: `cargo test --lib && cargo bench --bench line_render -- --noplot`
Expected: PASS plus a measurable improvement if Task 6 was necessary.

**Step 5: Commit**

```bash
git add src/multi_line.rs Cargo.toml
git commit -m "perf: use upstream batched multi-polyline api when available"
```

### Task 7: Validate the final budget and document the before/after numbers

**Files:**
- Modify: `README.md`
- Modify: `docs/plans/2026-03-07-line-rendering-performance.md`

**Step 1: Run the full verification set**

Run: `cargo test --lib`
Expected: PASS

Run: `cargo bench --bench line_render -- --noplot`
Expected: PASS and final measurements available.

**Step 2: Record the results**

Document:
- baseline and final median time for `multi_line_pan_1k_series`
- baseline and final median time for `line_hover_100_moves`
- whether Task 6 was needed
- observed LOD cache memory ceiling

**Step 3: Commit**

```bash
git add README.md docs/plans/2026-03-07-line-rendering-performance.md
git commit -m "docs: record line rendering performance results"
```
