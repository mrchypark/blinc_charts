# blinc_charts_frontend

Real Blinc web/WASM gallery for `blinc_charts`.

## Verify

These checks validate the gallery crate without opening a browser:

```bash
cargo check --manifest-path examples/blinc_charts_frontend/Cargo.toml
cargo test --manifest-path examples/blinc_charts_frontend/Cargo.toml
```

To verify the wasm target without launching a browser:

```bash
rustup target add wasm32-unknown-unknown
cargo check --manifest-path examples/blinc_charts_frontend/Cargo.toml --target wasm32-unknown-unknown
```

## Run

Prerequisites:

- `wasm-pack`
- a Rust toolchain with `wasm32-unknown-unknown` installed
- `wasm-pack` resolving to that toolchain's `rustc`

```bash
cd examples/blinc_charts_frontend
rustup target add wasm32-unknown-unknown
wasm-pack build --target web --release
./serve.sh
```

If `wasm-pack` reports that `wasm32-unknown-unknown` is missing even though
`rustup target list --installed` includes it, make sure `wasm-pack` is using a
rustup-managed `rustc`. Homebrew `rustc` installations need the wasm target
installed separately.

On systems without a POSIX shell, serve the directory with any static HTTP
server after `wasm-pack build`, for example:

```bash
python3 -m http.server 8000
```

The example intentionally does not commit font binaries. Blinc's WebGPU text
path needs actual TTF/OTF bytes registered at runtime; without a registered
font, chart geometry still renders but text labels may be absent. For a full
app, fetch or bundle a small font and call `WebApp::run_with_setup` to register
it before the first frame.
