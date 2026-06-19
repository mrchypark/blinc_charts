# blinc_charts

Canvas-first, high-performance interactive charts for the Blinc ecosystem.

## Status

This repository was split from `mrchypark/Blinc` (`crates/blinc_charts`) with history preserved.

## Dependencies

`blinc_charts` depends on `blinc_core`, `blinc_layout`, and `blinc_paint` from
`mrchypark/Blinc`. Chart rendering code uses `blinc_paint` for the paint-facing
primitive surface while keeping direct `blinc_core` usage for APIs that are not
currently re-exported by `blinc_paint`.

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

## Example Frontend

A complete Blinc web/WASM gallery lives in
[`examples/blinc_charts_frontend`](examples/blinc_charts_frontend). It uses the
real `blinc_charts` models, handles, linked chart APIs, and Blinc layout
elements.

```bash
cargo check --manifest-path examples/blinc_charts_frontend/Cargo.toml
cargo test --manifest-path examples/blinc_charts_frontend/Cargo.toml
```

To run it in a browser, build the wasm package and serve the example directory:

```bash
cd examples/blinc_charts_frontend
wasm-pack build --target web --release
./serve.sh
```

## Example Desktop

A native Blinc windowed gallery lives in
[`examples/blinc_charts_desktop`](examples/blinc_charts_desktop). It shares
host-testable sample/model/UI construction code and uses the real
`blinc_app::windowed::WindowedApp` entrypoint.

```bash
cargo check --manifest-path examples/blinc_charts_desktop/Cargo.toml
cargo test --manifest-path examples/blinc_charts_desktop/Cargo.toml
```

Run it from a desktop session with GPU/window access:

```bash
cargo run --manifest-path examples/blinc_charts_desktop/Cargo.toml --release
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
