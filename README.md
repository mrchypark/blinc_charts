# blinc_charts

Canvas-first, high-performance interactive charts for the Blinc ecosystem.

## Status

This repository was split from `mrchypark/Blinc` (`crates/blinc_charts`) with history preserved.

## Dependencies

`blinc_charts` depends on `blinc_core` and `blinc_layout` from `mrchypark/Blinc`.

## Usage

```toml
[dependencies]
blinc_charts = { git = "https://github.com/mrchypark/blinc_charts.git", branch = "main" }
```

## Development

```bash
cargo check
cargo test
cargo bench --bench line_render -- --noplot
```

## Performance Budgets

The line-rendering harness lives in `benches/line_render.rs`.
It currently uses an 8-second Criterion measurement window and 80 samples to stabilize
absolute timings. The 5% Criterion noise threshold only affects named-baseline comparison
reports; it does not change the absolute timing estimates.

Capture an intentional named baseline instead of relying on Criterion's implicit `base`
directory:

```bash
cargo bench --bench line_render -- --noplot --save-baseline main
```

Because the headline benchmark names changed in this harness revision, recapture your named
baseline once before relying on `--baseline main` comparisons again.

Compare later changes against that same baseline with:

```bash
cargo bench --bench line_render -- --noplot --baseline main
```

Primary timed gates from the current Criterion harness on the primary development machine at
`1280x720` are:

- `multi_line_pan_1k_series_10_frames` median benchmark time: `< 10 ms` for a 10-frame warm-cache pan sweep
- `line_hover_1k_moves` median benchmark time: `< 1.5 ms` for a 1,000-move hover sweep

These gates were re-measured on the renamed workloads above and are not directly comparable to
the older `multi_line_pan_1k_series` / `line_hover_100_moves` budgets.

Supplemental Criterion benches that help localize regressions:

- `multi_line_density_overview_10k_series`
- `lod_cache_query_512_points`
- `lod_cache_stitch_visible_edges_512_points`
- `line_small_window_raw_visible_11_points`

### Supplemental checks (not measured by Criterion harness yet)

- No local optimization phase should increase `RecordingContext::commands().len()` for the same rendered geometry
- `MultiLineChartModel` LOD cache memory should stay within a documented byte budget; current target: `<= 64 MiB`

Percentage improvement versus the captured baseline is useful secondary context, but it is not
the primary acceptance gate. If you only want fresh absolute numbers, remove stale Criterion
output first with `rm -rf target/criterion`. That also deletes named baselines, so recapture
them before using `--baseline main` again.

## License

Apache-2.0
