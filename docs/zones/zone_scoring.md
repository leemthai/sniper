# Zone Scoring Specification

LEE's NOTES: IS ANY OF THIS TRUE?

_Last updated: 2025-11-20_

This document describes how CVA zone scores are constructed and how they flow into downstream analytics (notably Journey Analysis).

For the journey pipeline that consumes these zones, see `docs/journeys/journey_spec.md` in the same directory.

## 1. Inputs and Scope

Zone scoring operates on a fully built `CVACore` instance and treats the **zone index** (0..N-1) as the primary axis. Price levels, candles, and ranges are only visible indirectly through pre-computed score vectors.

Key inputs:

- **`CVACore`** – encapsulates the per-zone score arrays for different `ScoreType`s.
- **Zone count** – fixed per CVA run (e.g., 100 zones over the active price band).
- **Auto-duration ranges** – a set of discontinuous `(start, end)` candle ranges, selected upstream based on the current price and `AutoDurationConfig`.
  - These ranges control which candles feed into CVA and thus the raw score arrays.
  - The zone-scorer itself only sees the *resulting* zone scores, not the ranges directly.

## 2. Combination Strategy (`CombinationStrategy`)

`CombinationStrategy` (in `src/analysis/zone_scoring.rs`) describes how to compress multiple normalized data sources into a single scalar per zone:

- **Average** – simple arithmetic mean of all source scores.
- **GeometricMean** – `(a * b * c)^(1/n)`; penalises low outliers more than `Average`.
- **Product** – straight product; very conservative, any low score collapses the result.
- **WeightedSum(Vec<f64>)** – application-specific weighting per data source (weights should sum close to 1.0).
- **Min** – “weakest link” view; the minimum score across all sources.
- **Max** – “optimistic” view; the maximum score across all sources.

`ZoneScorer` currently uses a fixed strategy wired up in code (subject to tuning). The combination logic itself is deterministic and side-effect free.

## 3. ZoneScorer API

`ZoneScorer` is a thin wrapper that holds:

- `data_sources: Vec<ScoreType>` – which signals to use.
- `strategy: CombinationStrategy` – how to combine them.

Important methods:

- `ZoneScorer::new(data_sources, strategy)` – construct explicitly.
- `ZoneScorer::from_set(HashSet<ScoreType>, strategy)` – convenience helper with deterministic ordering.
- `compute_scores(&self, cva_results: &CVACore) -> Option<Vec<f64>>` – main entry point:
  - Pulls raw vectors from `CVACore::get_scores_ref(score_type)`.
  - Normalises each to `[0, 1]` via `maths_utils::normalize_max`.
  - Applies the chosen strategy zone-by-zone.
  - Returns `None` if there is no usable data for any selected score type.

`compute_scores` is intended to be a pure function with no UI or logging concerns.

## 4. Higher-Level Zone Selection Helpers

On top of the combined score vector, a set of helpers pick *key* zones for different semantic roles.

### 4.1 Gradients

`calculate_zone_gradient(zone_scores: &[f64]) -> Vec<f64>` computes the absolute difference between adjacent scores, yielding a gradient vector of length `zone_scores.len() - 1`.

This vector is used as a proxy for “how spiky” the scores are around each zone.

### 4.2 Sticky / Consolidation Zones

`find_high_activity_zones_low_gradient` selects indices whose scores are both:

- In the **top X%** of scores (activity), and
- Surrounded by **low gradients** (consolidation rather than spikes).

The function:

1. Sorts `zone_scores` to compute a score threshold at `top_percentile`.
2. Computes gradients and a gradient threshold at `gradient_percentile`.
3. Returns all indices where:
   - `score >= score_threshold`, and
   - Both neighbouring gradients (if present) are `<= max_gradient`.

This produces candidate sticky zones.

### 4.3 Simple High-Activity Zones (e.g., Wicks)

`find_high_activity_zones(zone_scores: &[f64], top_percentile: f64)` ignores gradients:

1. Computes a score threshold at `top_percentile`.
2. Returns all indices with `score >= score_threshold`.

This is better suited for sharp, wick-like behaviour where we care only about magnitude, not smoothness.

### 4.4 Consolidation from Peaks

`find_consolidation_zones_from_peaks` is the most involved helper. It:

1. Uses `find_peaks::PeakFinder` to detect raw local maxima under constraints:
   - Minimum absolute height (`min_peak_height`).
   - Minimum prominence (`min_prominence`).
2. Manually inspects the **boundary zones** (0 and N-1) to compensate for peak-finder edge behaviour.
3. Sorts candidate peaks by strength and runs a **strength-based selection** with distance constraints:
   - Enforces a minimum index distance (`min_distance_fraction * zone_count`).
   - Retains weaker peaks only if they are within `strength_tolerance` of nearby stronger peaks.
4. Applies an **expansion** step around each retained peak:
   - Walks left/right up to `max_expansion_fraction * zone_count`.
   - Includes neighbours whose score is within `expansion_threshold` of the peak height.
5. Post-processes the set to **fill 1-zone gaps** when the middle zone’s score is at least `min_single_zone_gap_fill_pct * average(peak_left, peak_right)`.

The resulting set of indices is used as sticky/consolidation candidates for the UI and downstream analysis.

## 5. Multi-Pair and Journey Integration

- The **zone count** is fixed per CVA run (e.g., 100), irrespective of how many auto-duration ranges were used.
- Zone-scorer helpers operate solely on `Vec<f64>` of length `zone_count`.
- Multi-pair analytics (e.g., `MultiPairMonitor`, Journey Analyzer) treat the selected zones as abstract indices and query their corresponding prices from `CVACore`.

In other words, **low auto-duration range counts do not directly cap the number of zones**; they affect the underlying score distribution, which may then influence which indices cross the activity and gradient thresholds.

## 6. Debugging Hooks

Zone scoring has no built-in logging in release builds. For targeted diagnostics, use the following flag in `src/config/debug.rs`:

```rust
/// If non-empty, emit detailed zone-scoring debug output only for this pair.
/// Example: "PAXGUSDT". Use "" to disable.
pub const PRINT_ZONE_SCORING_FOR_PAIR: &str = "";
```

When compiled with `debug_assertions` and this const is set to a non-empty pair name (e.g., `"PAXGUSDT"`), `find_consolidation_zones_from_peaks` will print:

- Boundary peak insertions (left/right edges).
- Candidate peaks after boundary checks.
- For each candidate, whether it was allowed or rejected and **why** (boundary, strong enough, too weak, no nearby peaks, etc.).
- Expansion summaries for each peak (how many zones were added before/after).
- Gap-filling decisions (when a single-zone gap is filled between two peaks).

This output is gated by both `PRINT_ZONE_SCORING_FOR_PAIR` and `cfg(debug_assertions)` to keep production logs quiet.

## 7. Known & Suspected Issues

### 7.1 “0 or 1 key zone” on certain pairs

Empirically, some pairs (e.g., `PAXGUSDT`, `ZECUSDT`) have exhibited behaviour where the final sticky-zone selection yields **0 or 1 key zones**, even though:

- The auto-duration ranges cover thousands of candles (e.g., 4k+), and
- The configured `zone_count` is the same as for other pairs (e.g., 100).

Initial observations:

- These pairs sometimes show **very low auto-duration range counts** (e.g., ~12 ranges) compared with 200–300 ranges for more “busy” symbols.
- Low range count alone *should not* force 0–1 zones; the helpers only care about score/gradient distributions.
- However, highly truncated or oddly-shaped data can produce skewed percentiles and prominence thresholds, which in turn may cause:`
  - Only 1–2 peaks to cross `min_peak_height` / `min_prominence`.
  - Most candidate peaks to be rejected as too weak vs nearby stronger peaks.

### 7.2 Investigation Playbook

When a pair shows obviously bad behaviour (e.g., “only one sticky zone” while price action clearly visits multiple clusters):

1. **Set the debug flag** in `src/config/debug.rs`:
   ```rust
   pub const PRINT_ZONE_SCORING_FOR_PAIR: &str = "PAXGUSDT"; // or the problematic symbol
   ```
2. Rebuild/run in debug mode and reproduce the issue.
3. Capture the console output from `zone_scoring`:
   - Check how many raw peaks are detected.
   - Inspect why peaks are rejected (reason strings).
   - Verify expansion and gap-filling behaviour around key clusters.
4. Correlate with the **auto-duration stats** (ranges count, total candles) logged elsewhere to see whether the sample window is unusually narrow or fragmented.

If patterns emerge (e.g., boundary peaks often rejected incorrectly, or thresholds too strict for certain volatility regimes), tune the parameters:

- Lower/raise `min_peak_height` / `min_prominence`.
- Adjust `min_distance_fraction` and `max_expansion_fraction`.
- Revisit `min_single_zone_gap_fill_pct`.

Any such changes should be validated across multiple pairs to avoid overfitting to a single problematic symbol.

---

This document should be updated whenever:

- New score sources or combination strategies are introduced.
- Thresholds/percentiles used in the selection helpers are retuned.
- Additional debug instrumentation is added or removed.
