# Multi-Pair Monitor & Trigger Lifecycle

_Last updated: 2025-11-20_

This document explains how `MultiPairMonitor`, price streams, and `PairTriggerState` work together to decide **when** to recompute CVA and downstream analytics.

For the high-level motivation and roadmap, see `decoupling_plan.md`. For journey-specific usage of these triggers, see `journey_spec.md`.

## 1. Components

### 1.1 MultiPairMonitor

`MultiPairMonitor` (in `analysis/multi_pair_monitor.rs`) is responsible for:

- Tracking a `PairContext` per pair (trading model + current zone info).
- Processing price updates for **all** tracked pairs.
- Detecting **zone transitions** and emitting trading signals.

It is agnostic to *how* CVA is recomputed; it just consumes `TradingModel`s and live/simulated prices.

### 1.2 PairTriggerState

`PairTriggerState` (in `ui/app_triggers.rs`) encapsulates, per pair:

- `anchor_price` – price at which the last accepted CVA run was computed.
- `pending_price` – price that has moved sufficiently to warrant a new run but is not yet scheduled.
- `active_price` – price currently being processed by an in-flight job.
- `last_run_at` – timestamp of the last completed job.
- `is_stale` – whether the pair needs a recompute.
- `in_progress` – whether a job is currently running for this pair.
- `stale_reason` – human-readable string explaining *why* the trigger is stale.

It is responsible for enforcing:

- **Price hysteresis** – only mark stale when price moves more than a configured percentage away from `anchor_price`.
- **Debounce** – minimum wall-clock time between accepted recomputes.
- **Follow-ups** – when price keeps moving after a job starts, queue a **follow-up** job at the new price once the current one finishes.

### 1.3 LevelsApp Wiring

`LevelsApp` holds:

- `multi_pair_monitor: MultiPairMonitor` – analytics layer.
- `pair_triggers: HashMap<String, PairTriggerState>` – one trigger per pair.
- `price_stream: Option<PriceStreamManager>` – provides live prices per symbol.

Helpers in `ui/app_triggers.rs` manage:

- Trigger initialisation (`sync_pair_triggers`, `initialize_multi_pair_monitor`).
- Marking pairs as stale (`mark_pair_trigger_stale`, `schedule_selected_pair_recalc`).
- Draining the queue and launching async CVA jobs (`drain_trigger_queue`).

## 2. Price Flow and Trigger Updates

At a high level, each frame in `LevelsApp::update` does:

1. **Ensure price stream exists** and subscribed to all pairs.
2. **Ensure monitor initialised** once prices are available.
3. For each pair:
   - Fetch latest **display price** (live or simulated).
   - Feed price into `MultiPairMonitor::process_price_update`.
   - Pass the same price into the corresponding `PairTriggerState::consider_price_move`.

`consider_price_move`:

- Compares the new price against `anchor_price`.
- Returns `true` when the move exceeds the configured percentage threshold.
- The caller then records `pending_price` to be scheduled when allowed by debounce.

This flow ensures that **all pairs** can become stale and request recompute, even when they are not the currently selected symbol in the UI.

## 3. Scheduling & Async Execution

### 3.1 Scheduling

Scheduling is centralised via helper methods on `LevelsApp` (see `app_triggers.rs`):

- **`schedule_selected_pair_recalc(reason: &str)`** – convenience for the active pair.
- **`mark_pair_trigger_stale(pair: &str, reason: String, optional_price: Option<f64>)`** – mark any pair stale, optionally seeding its `pending_price`.
- **`drain_trigger_queue()`** – selects which stale pair (typically the selected one) should be scheduled next, respecting debounce and `in_progress`.

The scheduler never touches `MultiPairMonitor` directly; it only:

1. Decides *which* pair to recompute next.
2. Constructs appropriate `DataParams` for that pair.
3. Calls `start_async_calculation(params)` (see `app_async.rs`).
4. Updates the corresponding `PairTriggerState` to reflect an in-flight job (`in_progress = true`, `active_price = pending_price`, etc.).

### 3.2 Async Completion

When `poll_async_calculation` observes a completed `AsyncCalcResult`:

1. It clears `calculation_promise`.
2. Updates `DataState` with the new `Arc<CVACore>` and `last_calculated_params`.
3. Recomputes zone efficacy metrics.
4. Ensures `MultiPairMonitor` has an up-to-date `TradingModel` for the completed pair.
5. Notifies the associated `PairTriggerState` via:
   - `on_job_success()` (returns an optional queued `next_price`).
   - `on_job_failure(msg)` in error cases.
6. If a follow-up price was queued, it marks the pair stale again with a **follow-up** reason.
7. Calls `drain_trigger_queue()` to schedule the next job if appropriate.

This handshake keeps triggers, CVA results, and the multi-pair monitor aligned.

## 4. Relation to Decoupling Plan

The current trigger lifecycle implements key pieces of the `decoupling_plan.md` roadmap:

- **Price hysteresis & debounce** (Section 2.2) are encoded inside `PairTriggerState`.
- **Parameter change detection** (Section 2.3) is partially implemented by wiring UI events to `schedule_selected_pair_recalc(reason)` instead of directly recomputing.
- **Async orchestration** (Section 2.4) is realised by routing all recomputes through `drain_trigger_queue` and `start_async_calculation` instead of firing on every UI focus change.
- **Journey integration** (Section 2.6) will reuse the same completion events once we attach journey workers to CVA completion for each pair.

## 5. Debugging & Observability

### 5.1 Trigger Logs

When compiled with `debug_assertions`, `PairTriggerState` emits console logs such as:

- `Marked <PAIR> stale (<reason>), waiting on debounce/availability @ <price>`
- `queued follow-up for <PAIR> @ <price>`

These strings are intentionally human-readable so they can be eyeballed alongside cache logs like:

- `Cache HIT/MISS for <PAIR> with <N> ranges (<M> total candles) and <Z> zones.`

The combination of cache logs and trigger logs helps answer:

- Why was a recompute scheduled?
- Did debounce prevent a flurry of jobs?
- Are follow-up jobs being scheduled repeatedly for the same pair?

### 5.2 Known Gotchas

- **Legacy dirty flag:** the older `needs_recalc` flag has been removed; all recomputes must flow through `PairTriggerState` + `drain_trigger_queue`. Any remaining references are bugs.
- **Selected-pair bias:** `drain_trigger_queue` currently prioritises the selected pair to keep the UI responsive. This means off-screen pairs may lag slightly, which is acceptable for now but should be documented.
- **Simulation vs live price:** `get_display_price` mediates between simulation prices and live prices. Triggers see whichever view the UI is using; we do not currently maintain separate triggers per mode.

## 6. Future Work

1. **Richer prioritisation:** allow background precomputation for off-screen pairs when the system is idle.
2. **Explicit journey hooks:** fire journey analysis tasks when CVA for a pair completes, reusing the same trigger state and debounce protections.
3. **Configurable thresholds:** expose hysteresis/debounce thresholds via `config/analysis.rs` and the UI.
4. **Structured logging:** move from raw `println!` to a logging facade once a backend is introduced, preserving the same message shapes.

---

Update this document whenever the trigger state fields, scheduling policy, or logging surface changes.
