# Web Demo Goals
- Provide a browser-friendly showcase without requiring live Binance connectivity.
- Keep the native build fully featured; the demo is a restricted subset that ships with precomputed data.
- Minimize maintenance work when refreshing demo content.

## Web Version Release workflow
(Lee: what about committing latest version first? Or do that last in "publish to github" section... find out soon)
1. Decide on which pairs to put into the demo:
  - edit WASM_DEMO_PAIRS in `src/config/demo.rs` (note: must be pairs in pairs.txt)
2. From terminal, run this in order to rebuild latest binance data, then run helper to extract it:
  - crd (alias of: "cargo run --release; cargo run --bin make_demo_cache")
  - cbw (alias of: "cargo build --target wasm32-unknown-unknown")
3. Publish the updated wasm assets:
  1. delete 'dist' folder from project-root
  2. re-run `trunk serve` (if going to test locally)
4. Publish to GitHhub Pages:
  Just do a normaly github commit aand the updated site will be available within 5 minutes at: https://leemthai.github.io/zone-sniper/
(This works because of .github/workflows/deploy.yml and GitHub Actions.)

## `Trunk Serve` Full docs: https://crates.io/crates/trunk/0.9.1
runs by default at:
- http://127.0.0.1:8080
trunk build
trunk watch - same as build but watches for changes to files and automates the build process in the background
trunk serve - same as watch but also spawns a web serve.
trunk clean - cleans up any build artifacts from earlier builds.

## If we get issues with Trunk Serve itself:
1. locate current trunk procesees:
  sudo lsof -i :8080
2. kill them (they will all have same PID)
  sudo kill <PID>

# Debugging Web Version in Chrome
Ctrl+Shift+J to launch DevTools

### Web Version Data Strategy
1. **Full cache**: continue to generate `kline_data/kline_30m_v4.bin` with the complete pair list for the desktop app.
2. **Demo cache**: the helper writes `kline_data/demo_kline_30m_v4.bin` (prefixing the source filename) so the interval/version suffix is preserved for clarity. The wasm build embeds this exact file via `include_bytes!`, controlled by `WASM_DEMO_CACHE_FILE`.
3. **Helper CLI**:
   - Create a small binary (e.g. `cargo run --bin make_demo_cache`) that loads the full cache via `SerdeVersion`, filters to the hard-coded pair/interval list, and writes `demo_<original>.bin` (e.g. `demo_kline_30m_v4.bin`) with the existing bincode serializer.
   - Regeneration flow whenever new data is desired:
     1. Run the existing workflow you already use to refresh `kline_data/kline_30m_v4.bin` (e.g. your CLI command or script).
     2. `cargo run --bin make_demo_cache`
     3. `cargo build --target wasm32-unknown-unknown --features wasm-demo`

### WASM Loader Changes
- Introduce `src/config/demo.rs` to host demo-only constants (`WASM_MAX_PAIRS`, `WASM_DISABLE_NETWORKING`, `WASM_KLINE_BUNDLE_DIR`).
- For `target_arch = "wasm32"`, wire the data loader to:
```rust
const DEMO_CACHE_BYTES: &[u8] = include_bytes!(
    concat!(env!("CARGO_MANIFEST_DIR"), "/kline_data/", WASM_DEMO_CACHE_FILE)
);
let cache: CacheFile = bincode::deserialize(DEMO_CACHE_BYTES)?;
```
  This keeps the browser build offline and deterministic. The native build still uses the full cache or live Binance APIs.
- Enforce the pair cap by truncating `TimeSeriesCollection.series_data` to `WASM_MAX_PAIRS` immediately after deserialization.
