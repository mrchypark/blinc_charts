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

The baseline harness for line rendering lives in `benches/line_render.rs` and should be
captured with:

```bash
cargo bench --bench line_render -- --noplot
```

Primary timed gates from the current Criterion harness on the primary development machine at
`1280x720` are:

- `multi_line_pan_1k_series` median benchmark time: `< 8 ms` for steady-state samples after Criterion warm-up
- `line_hover_100_moves` median benchmark time for the full 100-move hover sweep: `< 2 ms`

### Supplemental checks (not measured by Criterion harness yet)

- No local optimization phase should increase `RecordingContext::commands().len()` for the same rendered geometry
- `MultiLineChartModel` LOD cache memory should stay within a documented byte budget; current target: `<= 64 MiB`

Percentage improvement versus the captured baseline is useful secondary context, but it is not
the primary acceptance gate.

## License

Apache-2.0
