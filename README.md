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
cargo check --benches
cargo bench --bench cartesian_render -- --noplot
```

## Performance Budgets

Benchmarks are now split by concern instead of living in a single `line_render` harness.

- `cartesian_render`
- `cartesian_interaction`
- `family_render`
- `plot_compile`
- `micro_hotpaths`

Use named baselines instead of Criterion's implicit `base` directory:

```bash
cargo bench --bench cartesian_render -- --noplot --save-baseline main
cargo bench --bench cartesian_interaction -- --noplot --save-baseline main
cargo bench --bench family_render -- --noplot --save-baseline main
cargo bench --bench plot_compile -- --noplot --save-baseline main
cargo bench --bench micro_hotpaths -- --noplot --save-baseline main
```

Compare later changes against the same captured baseline:

```bash
cargo bench --bench cartesian_render -- --noplot --baseline main
cargo bench --bench cartesian_interaction -- --noplot --baseline main
cargo bench --bench family_render -- --noplot --baseline main
cargo bench --bench plot_compile -- --noplot --baseline main
cargo bench --bench micro_hotpaths -- --noplot --baseline main
```

Absolute timing budgets and benchmark interpretation live in [BENCHMARKS.md](BENCHMARKS.md).

## License

Apache-2.0
