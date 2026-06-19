# blinc_charts_desktop

Native Blinc windowed gallery for `blinc_charts`.

This example keeps the chart inventory and UI builder in host-testable library
code, then uses the pinned Blinc desktop APIs in the binary entrypoint:
`blinc_app::windowed::WindowedApp` and `blinc_app::WindowConfig`.

## Check and Test

```bash
cargo check --manifest-path examples/blinc_charts_desktop/Cargo.toml
cargo test --manifest-path examples/blinc_charts_desktop/Cargo.toml
```

The tests also build the gallery coverage matrix used for mini-model eval
prompts: every supported chart family gets example-only task/evidence text, and
every documented variant x interaction row is compiled into a public
`blinc_charts` element builder. They do not call an AI model or open a window.

## Mini-Model Eval

```bash
python3 scripts/run_mini_gallery_eval.py --dry-run
OPENAI_API_KEY=... python3 scripts/run_mini_gallery_eval.py
```

The runner exports the desktop coverage matrix, gives the model only gallery
snippets/variant notes/interaction notes, writes generated Rust cases under
`target/blinc_charts_mini_eval`, then checks them with `cargo check --tests`.
Results are written to `summary.json`, `results.jsonl`, and `cargo_check.log`.

## Run

```bash
cargo run --manifest-path examples/blinc_charts_desktop/Cargo.toml --release
```

Native runs require a desktop session with GPU/window access. Headless CI
machines may be able to compile the native stack, but should not try to run the
windowed app.

No bundled font assets are required; the example relies on Blinc's platform font
loading.
