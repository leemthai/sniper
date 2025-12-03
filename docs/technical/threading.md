 # Threading - my personal thoughts:
    As I understand it, calculations are run in a background thread. This is important because it allows the UI to remain responsive while the calculations are being performed.
    This needs further analysis. In particular, what is the "source of truth" of system data? I never did quite understood how that worked.
    One weirdness about current threading is the 'cancel' option which seems to be still there, but should be completely removed.
  

## Threading & concurrency overview:

1. Global Tokio runtime in main
  Application bootstraps a single multi-thread Tokio runtime to fetch initial data, then uses it to spawn an async cache-write task so UI startup isn’t blocked 
  `src/main.rs`

2. Async cache I/O
  1. Disk-bound cache reads/writes run inside tokio::task::spawn_blocking, isolating CPU-heavy bincode work from the async reactor 
  `src/data/timeseries/serde_version.rs`
  2. Errors from the blocking task are surfaced back to the caller, so panics or deserialization issues don’t silently fail.

3. Binance API ingestion
  1. Batch klines fetches use tokio::spawn for each pair/interval combination and join_all to await completion, aggregating errors before proceeding 
  `src/data/timeseries/bnapi_version.rs`
  2. Rate-limit backoff is handled asynchronously (sleeping within async context); no extra threads are spawned there.

4. Live price streams
  1. PriceStreamManager::subscribe_all now launches a single background thread that connects to Binance’s combined mini-ticker stream for all subscribed symbols (see `src/data/price_stream.rs`).
    1. the worker uses one Tokio runtime with reconnection/backoff logic to keep the combined socket alive.
    2. Shared state (prices, connection_status, suspended) remains guarded with Arc<Mutex<...>> so UI readers stay thread-safe while the async loop mutates values.
    3. Benefit: drastically fewer threads/runtimes versus the old per-symbol model, simplifying lifecycle management and avoiding unbounded resource growth.

5. UI calculations
  1. Long-running CVA computations run in background threads via Promise::spawn_thread, with cancellation through Arc<AtomicBool> and progress reporting updated on the UI thread `src/ui/app.rs`
  2. Results are stored in Arcs to share safely with the UI once complete.


##  Key observations & recommendations:

1. Price stream consolidation achieved: the combined-stream worker now handles every subscribed symbol on a single Tokio runtime/thread. Future work could fold it into the global runtime when we wire structured shutdown hooks.
2. Provide lifecycle management for spawned threads (price streams and calculation workers) so the app can terminate cleanly or cancel them explicitly.
3. Audit mutex usage around price data to ensure UI reads don’t block for long; if contention appears, explore RwLock or channels.

No other threading primitives (e.g., explicit JoinHandle::join) appear elsewhere; concurrency is primarily through Tokio tasks, spawn_blocking helpers, and controlled background threads.


## UI Calculations

### Key observations

1. **Promise-based worker threads.** `start_async_calculation` uses `Promise::spawn_thread` with a cloned `ZoneGenerator`, `TimeSeriesCollection`, and `DataParams`. The UI thread holds the `Promise`, polls it during `update`, and swaps the completed `Arc<CVACore>` into `DataState` once `ready()` returns. This keeps egui responsive while long CVA sweeps run.
2. **Trigger discipline via `needs_recalc`.** Recalcs are only kicked off when `needs_recalc` is true and no other promise is outstanding. Inputs (pair, zone count, decay, slice ranges, price range) flow through `DataParams::from_app`, so parameter changes are explicit and comparable.
3. **Slice and price caching guardrails.** `computed_slice_indices` and `last_price_range` are shared across sync/async boundaries. We now gate recalcs on actual slice changes and stash the last params (`last_calculated_params`) to prevent redundant work.
4. **Cancellation hooks exist but are blunt.** `cancel_calculation` merely trips an `AtomicBool`, drops the promise, and resets UI flags. We rarely exercise it, but it’s still the only line of defence if users mash keys mid-run.
5. **Post-processing fans out on the UI thread.** After a promise resolves we synchronously compute secondary metrics (zone efficacy, monitor insertion). This means one slow follow-up section can still block the UI, so further offloading may be needed.

### Challenges & stress points

1. **Stale results after input switches.** When the selected pair/params change before a worker finishes, the background thread can still return data keyed to the old inputs. Without guardrails we previously displayed the old CVA for the new pair.
   - Mitigations we’ve adopted:
     - Compare the completed `params` against the app’s current selection before accepting the result.
     - Invalidate cached CVA/zone efficacy (`clear_zone_efficacy`, `needs_recalc = true`) whenever inputs change.
     - Reset `last_calculated_params` on pair change so “params unchanged” short-circuits don’t reuse stale data.
     - Optionally call `cancel_calculation` when a disruptive change (pair switch, data refresh) happens, so the old worker bails early.
2. **Race between recalculation triggers and price ticks.** Live price updates used to flip `needs_recalc` on every tick, spawning overlapping workers and thrashing caches. We curbed this by only triggering recalcs when slices/params truly change and by decoupling price stream updates from CVA recomputation.
3. **Cache pollution on partial failures.** If a worker errors halfway through, we must ensure `DataState` stays coherent (clear results, surface the error) so that subsequent retries receive a clean slate.
4. **Shared state cloning costs.** Cloning large `TimeSeriesCollection` and `ZoneGenerator` into every worker is convenient but heavy. We should explore `Arc` sharing plus interior mutability control if the scaling becomes painful.
5. **Lack of structured cancellation lifecycle.** Dropping the promise works today, but we have no join handles or deterministic shutdown for in-flight workers when the app exits or hot reloads.

### Where async will grow next

- **Journey analysis sweeps.** The multi-pair journey runners will reuse the same `Promise` pattern: capture immutable inputs, spawn a worker per batch, surface structured outputs back to egui.
- **Per-pair decay sweeps.** The manual “B” trigger demonstrated how to farm out scoring work without blocking the UI. As we reintroduce automatic calibration, the same guardrails (param hashing, stale-result protection) need to wrap those tasks.
- **Improved caching layers.** Future cache hits (CVA, calibration, journey outcomes) should be keyed by explicit parameter structs and guarded with equality checks before we short-circuit the pipeline.
