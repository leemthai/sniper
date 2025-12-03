# Journey Analysis Specification

_Last updated: 2025-11-19_

## Lee's Important update: Some parts of this doc are EXTREMELY MISLEADING.
This doc mistakenly believes the starting point of a trade is a zone. That is not true. The starting point of a trade is the current price of the asset being traded. The END price of a trade is one of the nearby zones.

That is why this document should be read in conjunction with `docs/journeys/theoretical_model.md` which describes the `Stage 0` which needs to be implemented in order to turn the current model into the model originally envisioned.

End of Lee's update -----------------------------

For a more formal description of the underlying model (events, random variables, and conditional distributions), see the companion document `docs/journeys/theoretical_model.md` in the same directory.

## 0. Journey terminology (local glossary)

- **Attempts** – number of historical journey runs we simulated from the current setup (start price, target, Time Horizon, stop-loss %). Each attempt ends as exactly one of: Success, Timeout, or Stopped-out.
- **Success / Timeout / Stopped-out** – mutually exclusive journey outcomes. A Success ("SR") reaches the target before Time Horizon and before the stop-loss. A Timeout has not reached the target by the Time Horizon but has also not violated the stop-loss. Stopped-out means price moved against us far enough to hit the configured stop-loss %.
- **Time Horizon** – (often known as "holding period") the maximum duration we allow a journey to run before we treat it as a Timeout. Tightening this knob (e.g. 64d → 7d) usually converts some slow historical successes into timeouts, lowers the overall success % but makes time‑to‑target stats (median / p90) shorter, while leaving the total number of attempts and the stop‑loss behaviour largely unchanged.
- **Time-to-target (TTT)** – for successful journeys, the time taken to reach the target, usually expressed in days. In the UI we summarise this with median and p90 values ("p90" = 90th percentile), so you can see how quickly most successful runs complete.
- **Drawdown / worst loss** – the worst adverse move experienced during a journey before it resolves. At the run level we track `max_drawdown_pct`; in summaries we report worst-case loss and average max drawdown to characterise how painful the path is, even for trades that eventually succeed.
- **Expected annualised return (EVA)** – the expected value of taking this journey repeatedly, once per historical opportunity, expressed as an annualised percentage. Combines frequency, win/loss sizes, and typical journey durations into a single "is this worth trading?" scalar.
- **Kelly criterion** – optional metric derived from the expected value inputs that suggests an aggressive position size fraction to maximise long-term growth. Present only when the underlying assumptions (win/loss signs and probabilities) are healthy enough to make it meaningful.

## 1. High‑level intent (narrative) (WARNING. THIS EXPLAINS CURRENTLY IMPLEMENTED Stage-1, which is NOT ultimately what we want to implement. We want Stage-0)

- At a high level, a *journey* asks a simple trading question: "If price is currently sitting in this sticky key‑zone, what usually happens next over the Time Horizon and stop-loss % I actually care about?" For each key‑zone we treat the current zone edge as a hypothetical entry and replay many historical runs that started from a similar price. Each run either hits the target (success), hits the stop (stopped out), or times out because it took too long. From this we estimate how often the setup works, how bad it is when it fails, and how long it typically takes to resolve.

- The goal is *not* to predict exact future prices tick‑by‑tick. Instead, journeys try to answer: "Is this zone historically a good place to take this kind of trade, given my chosen Time Horizon and stop-loss %?" The key summary statistics in the **Journey Outcomes** UI therefore focus on:

- Attempts and outcome mix – shown as `attempts N (successes X% | timeouts Y% | stops Z%)` so you see both how many runs we have and how often they end in each way.
- Time‑to‑target (TTT) – median and p90 days for successful journeys ("how fast do wins usually resolve, and how slow are the slow ones?") at both current‑pair and across‑pairs levels.
- Expected annualised return (EVA) – how attractive the payoff has been when you keep taking this trade over time.
- Risk metrics – worst loss and average drawdown during the run, to capture how painful the path can be even when setups eventually succeed.

Counts like **total attempts** tell you how much history backs up those estimates, while the outcome percentages and TTT/timeout behaviour tell you whether the trade typically resolves fast enough and cleanly enough to be worth considering.

Right now the system can tell you, per pair and per key‑zone, "how this particular journey has behaved in the past" under fixed global knobs (Price Horizon, Time Horizon, stop-loss %, tolerance). It does *not* yet do position sizing or portfolio optimisation for you, and it does not guarantee that the future will look like the past. As we iterate, the intent is to make the stats more actionable (e.g. clearer links between journey metrics and Kelly/position size, better visual cues when a zone looks attractive but under‑sampled, or when aggressive settings are causing mostly timeouts) rather than more complicated. Treat current outputs as a structured way to interrogate historical behaviour around sticky zones, not as a fully‑automated trading system.

For details on how key zones are scored and selected, see `docs/zones/zone_scoring.md`. For the multi-pair trigger orchestration that feeds journeys and other downstream analytics, see `docs/technical/multi_pair_triggers.md`.

For companion information about the user-facing controls that feed these journeys, see `docs/ui/trade_ui.md`.

## 2. Completed Functionality

### 2.1 Core Data Structures

- `Outcome` – success, timeout, or stop-loss (`StoppedOut { adverse_price }`).
- `JourneyOutcome` – per-journey record containing start price, outcome, `elapsed_days`, optional `days_to_target`, drawdown and final price.
- `JourneyParams` – analysis inputs, including tolerance, max window, stop-loss %, and Kelly toggle.
- `JourneyRequest<'a>` – wrapper used when targeting specific zones.
- `JourneyExecution` – envelope returned for each analyzed zone.
- `JourneyStats`, `ExpectedValue`, `RiskMetrics` – aggregated metrics over all outcomes.

### 2.2 Analysis Flow

1. **Context lookup** – `JourneyAnalyzer::analyze` locates matching OHLCV series via `find_matching_ohlcv`.
2. **Price matching** – `match_start_prices` finds historical candles whose close price is within `JOURNEY_START_PRICE_TOLERANCE_PCT`.
3. **Simulation loop** – `evaluate_price_matches` iterates forward candle-by-candle to detect:
   - Target hit in the anticipated direction (success).
   - Stop-loss breach using `JOURNEY_STOP_LOSS_PCT` (failure, `StoppedOut`).
   - Exhaustion of the allotted window (timeout).
4. **Duration tracking** – each outcome records `elapsed_days` based on the actual number of steps processed. Successes additionally record `days_to_target`.
5. **Metrics** – `compute_stats` aggregates ROI, annualises gains/losses with a linear model, computes Wilson confidence intervals, risk metrics, and expected value. Kelly criterion is calculated only when `compute_kelly` is true.

### 2.3 Configuration Surface (conceptual)

The main knobs that change journey behaviour are:

- **Candle interval** – which resolution of price data is used (e.g. 30‑minute candles). This controls how many steps we can take inside a fixed Time Horizon window.
- **Time Horizon (days)** – the maximum duration we allow a journey to run before we treat it as a Timeout. This is controlled via the Time Horizon slider in the Data Generation panel.
- **Start price tolerance (%)** – how tightly we match the **current live price** when searching for historical starting points around the sticky zone edge. A smaller tolerance means we only use historical candles whose close is very close (e.g. within ±0.5%) to today’s price; a larger tolerance accepts a looser band (e.g. ±2–3%), giving more samples but less strict price similarity.
- **Stop-loss %** – how far price can move against us before we record a Stopped‑out outcome.

Exact constant and field names for these live in the code; this document focuses on how changing each knob should affect journeys conceptually.

## 3. Outstanding Work (high level)

These items describe *behavioural gaps* rather than specific code changes.

1. **Configurable runtime controls** – bring more of the important knobs (especially stop-loss %, start-price tolerance, and any Kelly toggle) into the UI so they can be explored without editing code.
3. **Timeout handling** – make recorded timeout durations more faithful to what actually happened (rather than defaulting to the max window when no progress was made).
5. **Zone efficacy study** – deepen the sticky-zone analytics (see Section 8 and `docs/zones/zone_scoring.md`) so zones can be judged not only by journeys but also by dwell behaviour and transitions.
6. **Zone construction refinement** – revisit the sticky-zone construction / consolidation algorithms using annotated zone maps ("zones I like", "zones I dislike", "missing zones", "merge/split suggestions") as ground truth to better align automatic zones with human intuition.
7. **Validation harness** – add a lightweight validation loop so we can measure whether journey probabilities and EVs generalise beyond the training window.
8. **Testing and error surfacing** – add more end-to-end checks around typical success/timeout/stop-loss scenarios and surface failures in a more visible way than console logs.

## 4. Future Enhancements (ideas, not promises)

1. **Stop-loss flexibility** – explore non-symmetric or dynamic stop-loss rules (e.g., trailing or volatility-aware) and how they change journey behaviour.

## 6. Recalculation Triggers (conceptual)

- **Zone changes** – whenever key zones are rebuilt (e.g., due to Price Horizon or decay tweaks), prior journey stats for those zones are no longer trustworthy and should be refreshed.
- **Time Horizon changes** – tightening or loosening the Time Horizon slider reclassifies some historical runs between Success and Timeout, so journeys are re-run with the new window.
- **Stop-loss changes** – adjusting the stop-loss % changes which paths count as Stopped-out, affecting both risk metrics and outcome mix.
- **Material price moves** – when live price drifts far from the samples used in the last run, journeys should be refreshed so “current price ≈ historical starting price” remains true.

Implementation details for queues, guards, and failure handling live in `docs/technical/multi_pair_triggers.md`

## 7. Validation Roadmap (sketch)

- Reserve a small hold-out window of recent data and only use older data to fit journey probabilities.
- Replay journeys through the hold-out window and compare realised outcomes with predicted success probabilities.
- Track simple scores (e.g., calibration or Brier-style errors) over time so we can see whether changes to journeys improve or degrade forecast quality.

## 8. Sticky Zone Efficacy (overview) (ALL THIS DWELL STUFF IS PROBABALY REDUNDANT BECAUSE IT ASSUMES WE START IN ZONES, WHICH IS NOT TRUE)

  Today sticky-zone analytics complement journeys by answering “how does price *sit* in these zones?” rather than “what happens if I trade from here?”
  
  - **Price and time occupancy** – how much of the analysis band is covered by sticky zones, and what share of candles spend time inside them.
  - **Dwell duration summary** – how long typical visits last in candles (runs, median, p90, max), surfaced in the status bar when debug flags are enabled.
  - **Transition hints (debug)** – basic summaries of where price tends to go after leaving one zone and how long that typically takes.
  
  > Future work in this area focuses on using dwell/transition behaviour to refine which zones count as truly “sticky” and how that should influence journey interpretation.
