# Historical Price Journey Analysis (HPJA) – Theoretical Model

This document describes the *current* theoretical model behind the journey engine. It is intentionally simple, explicit, and matches what the code does today, not what we might want in the future.

---

## 1. Core Idea

For the trading system we ultimately care about, the primary object of interest is:

> "From today’s live price, how do paths behave when we aim to **take profit in a sticky zone** (treated as a natural price target)?"

This is the **Stage 0** model (live price → sticky zone as take-profit), described as a future extension in Section 9.1. It treats sticky zones as *targets* from live price.

The engine that currently exists in code implements a related but different question, which we call **Stage 1**:

> “Given that price is in zone Z at time t, and we open a trade with these rules, what is the empirical distribution of outcomes from that zone context?”

Stage 1 studies what tends to happen **after price is in a particular zone**, under fixed trading rules (target, stop, time horizon). It does **not** simulate from today’s live price into the zone; instead it conditions on historical moments when price was already inside the zone.

Stage 1 is best understood as:

- An **event-based backtest** (event = "price is in zone Z").
- An empirical estimate of **first-passage / hitting-time behaviour** from zone Z to target vs stop within a fixed time horizon.

We call this Stage-1 construction **Historical Price Journey Analysis (HPJA)**. It can be kept as an auxiliary, conditional analysis on top of the Stage‑0 live→zone model, or simplified later if it proves less useful in practice.

---

## 2. Basic Objects and Notation

We work with a single trading pair and a historical price series sampled at fixed intervals (e.g., candles), indexed by integer time `t`.

- Let `P_t` be the mid price (or close) of the pair at time `t`.
- Let `Δt` be the fixed time step between `t` and `t+1` (one candle).
- Let `Z = [Z_low, Z_high]` be a **zone** in price space.
- Let `dir` be the **direction** of the trade:
  - `dir = +1` for a bull journey (long from zone upwards).
  - `dir = -1` for a bear journey (short from zone downwards).
- Let `tol` be the **start-price tolerance** (a percentage band) used to decide whether `P_t` counts as “inside” the zone.
- Let `T_h` be the **maximum time horizon** (e.g., 90 days), measured in wall-clock time or number of steps.
- Let `SL_pct` be the **stop-loss percentage** from the entry price.
- Let `TP` be the **target level** in price space determined by the zone and direction.

Given these, we construct hypothetical trades and measure their outcomes.

---

## 3. What Is an "Attempt"?

An **attempt** is a single **hypothetical trade** starting from a historical candle where price was already in the zone.

Formally, for a given zone `Z` and direction `dir`:

1. **Identify candidate start times**:
   - For each historical index `t`, we check whether price at time `t` is within the zone, accounting for the start-price tolerance band.
   - If `P_t` is inside `Z` (within `tol`), then `t` is a **zone-hit event**.

2. **Each zone-hit event becomes one attempt**:
   - At such a time `t`, we imagine opening a trade with:
     - Entry price `E = P_t`.
     - Direction `dir` (long/bull or short/bear).
     - Stop-loss and target determined from `E`, `SL_pct`, and the zone.
     - Maximum holding time `T_h`.

3. **We then simulate the trade forward from `t`** until one of:
   - Price hits the **target** (`TP`).
   - Price hits the **stop-loss** (`SL`).
   - Time horizon `T_h` is exceeded.

So in this model:

- **1 attempt = 1 hypothetical trade** starting at a specific historical candle where price was in the zone.
- The **number of attempts** for a zone is the **number of historical zone hits** in the dataset that meet the entry conditions.

This is why multiple zones in a journey can share the same number of attempts: they all inherit the same set of qualifying zone hits under the current logic.

---

## 4. Trade Rules Per Attempt

Given a particular attempt starting at index `t_0` (where price is in zone `Z`):

- **Entry price**: `E = P_{t_0}`.
- **Direction**:
  - Bull (`dir = +1`): we are long from zone upwards.
  - Bear (`dir = -1`): we are short from zone downwards.

- **Target price** (`TP`): determined by the zone and direction, e.g.:
  - Bull: target above the zone.
  - Bear: target below the zone.

- **Stop-loss price** (`SL`): percentage away from `E`:
  - Bull: `SL = E * (1 - SL_pct)` (downside stop).
  - Bear: `SL = E * (1 + SL_pct)` (upside stop for a short).

- **Time horizon** (`T_h`): maximum allowed duration from `t_0`.

We then inspect the future path `{P_t : t >= t_0}` up to the time-horizon cutoff.

---

## 5. Journey Outcomes Per Attempt

For each attempt, we classify the outcome into one of three categories:

1. **Success** – target hit first:
   - There exists some `t_success` with `t_0 < t_success <= t_0 + T_h` such that:
     - For a bull journey: `P_t` crosses or touches `TP` before hitting `SL`.
     - For a bear journey: `P_t` crosses or touches `TP` (downwards) before hitting `SL`.

2. **StoppedOut** – stop-loss hit first:
   - There exists some `t_stop` with `t_0 < t_stop <= t_0 + T_h` such that:
     - For a bull journey: `P_t` crosses or touches `SL` before hitting `TP`.
     - For a bear journey: `P_t` crosses or touches `SL` (upwards) before hitting `TP`.

3. **Timeout** – neither target nor stop is hit by the horizon:
   - No `t` in `(t_0, t_0 + T_h]` where `TP` or `SL` is reached.
   - The trade is closed at `t_timeout = t_0 + T_h` with whatever unrealized P&L exists at that time.

These outcomes are mutually exclusive and exhaustive for each attempt.

Over all attempts for a zone, we compute:

- `N_total` – total attempts.
- `N_success`, `N_timeout`, `N_stopped_out`.
- Percentages:
  - `p_success = N_success / N_total`.
  - `p_timeout = N_timeout / N_total`.
  - `p_stopped_out = N_stopped_out / N_total`.

These are the **attempt percentages** shown in the UI.

---

## 6. Direction-Aware Returns and EVA

For each attempt, we can define a direction-aware return:

- Let `R_raw` be the percentage return of the trade from entry to exit, before annualisation.
- For a **bull (long)** trade:
  - `R_raw = (P_exit / E) - 1`.
- For a **bear (short)** trade:
  - Effectively: `R_raw = (E / P_exit) - 1`, i.e., we profit when price falls.

This ensures that:

- Positive `R_raw` means profitable for the configured direction.
- Negative `R_raw` means losing for the configured direction.

The engine then computes annualised return metrics (EVA) from these per-attempt returns and holding times, treating them as **samples from a strategy that always trades this zone under these rules whenever the entry condition is met**.

Important conceptual point:

- EVA is an **estimate of performance of a specific trading rule**:
  - "Whenever price is in this zone, trade it with this TP/SL/horizon."
- It is **not** a statement about a single trade from today, but about a *strategy applied consistently* over all historical occurrences.

---

## 7. What HPJA Is and Is Not

### 7.1 What It Is

- An **event-based conditional backtest**:
  - Condition on: `E = {price is in zone Z at time t}`.
  - Measure: distribution of outcomes when trading from that event with fixed rules.

- An empirical approach to **first-passage / hitting-time questions**:
  - How often does price reach target before stop within `T_h` from this zone?
  - What does the P&L distribution look like conditional on being in this zone?

- A way to summarise a zone as a **trading setup**:
  - Attempts: sample size.
  - Success/timeout/stop percentages.
  - Returns and EVA.
  - Drawdowns and time-to-target statistics.

### 7.2 What It Is Not (Today)

- It does **not** simulate from **current live price into the zone** and then to target.
- It does **not** condition on broader **regime information** (trend, volatility regime, macro context).
- It does **not** (yet) explore the **full surface** of TP/SL/horizon systematically; we run with a chosen configuration.

These are all potential extensions, not contradictions of the current model.

---

## 8. Theoretical Legitimacy

From a theoretical perspective, HPJA is a standard and defensible construction:

- It is essentially an **event study** on price paths:
  - Event: "price is in zone Z".
  - Outcome variables: hit target first, hit stop first, or timeout; plus returns and drawdowns.

- It estimates **conditional distributions** based on historical data:
  - `P(outcome | price_in_zone_Z, config)`.
  - `E[return | price_in_zone_Z, config]`.

- It is compatible with many underlying stochastic price models (e.g. random walks, diffusions) without committing to any one; it simply uses empirical paths.

Where it becomes theoretically fragile is the usual set of backtest issues:

- **Stationarity**: assuming that the relationship `price in this zone -> future behaviour` is stable across time.
- **Selection bias and overfitting**: if zones or parameters are tuned on the same data used to evaluate them.
- **Regime shifts**: structural changes in the market that break past patterns.

These are not unique to HPJA; they are the standard caveats for any historical strategy analysis.

---

## 9. Directions for Extension (Conceptual Only)

Without changing the current engine, we can see clear upgrade paths.

### 9.1 Stage 0 – From live price to sticky zone (original intent)

The original mental model for journeys treats **sticky zones as targets from the current live price**, not as entry regions. Formally, this suggests inserting a **Stage 0** in front of the current HPJA pipeline:

- Let `P_live` be the current live price at time `t_live`.
- Let `Z = [Z_low, Z_high]` be a sticky zone somewhere above or below `P_live`.

Stage 0 asks:

> "Starting from `P_live`, how often and how quickly does the path reach zone `Z`, and what is the path’s pain profile along the way?"

Conceptually, we would:

1. Identify historical candles where `P_t` is **similar to the current live price** (within some tolerance band), *regardless of whether `P_t` is in any zone*.
2. From each such start point, track the future path until one of:
   - The price first **enters zone `Z`** (first-passage into the zone).
   - A configured **time horizon** (for Stage 0) is exceeded.
   - An optional **guardrail** is hit (e.g., price moves too far away, making the scenario irrelevant).
3. Classify and summarise:
   - Fraction of paths that ever hit `Z`.
   - Time-to-zone (median / p90) for paths that do.
   - Maximum adverse move against the intended direction before entering `Z`.

This yields a **live → zone** model that treats sticky zones as *targets* from the current context.

If we denote Stage 0 by a random time `T_Z` (time to first enter zone `Z` from `P_live`, possibly `∞` if never reached within the window), then Stage 0 is about estimating the distribution of `T_Z` and related path-dependent quantities.

### 9.2 Composing Stage 0 and Stage 1

The current HPJA engine is naturally interpreted as **Stage 1**:

- Condition on: `E_zone = {price is in zone Z at time t}`.
- Measure: distribution of outcomes when trading from `E_zone` with fixed TP/SL/horizon.

When Stage 0 is added, the conceptual workflow becomes:

1. **Stage 0 (live → zone):**
   - `P_live  →  Z` (does price visit this sticky zone at all, and on what timescale / with what risk?).
2. **Stage 1 (zone → target):**
   - `Z  →  TP/SL/horizon` (given we are in this sticky zone, how do trades from here behave?).

This two-stage structure matches the original intent:

- **Intent:** Sticky zone is a **destination** from live price, then possibly a launchpad for more specific trades.
- **Current implementation:** Only Stage 1 is implemented; Stage 0 is a future extension.

When both are present, you can answer questions like:

- "From today’s price, how plausible is it that we even reach this key zone in a reasonable time?"
- "Conditional on getting there, is trading from that zone historically attractive?"

The rest of this document (Sections 2–8) describes the implemented **Stage 1** HPJA model. Stage 0 is documented here as a **theoretical extension** that can be built on top without discarding the existing code or concepts.

### 9.3 Regime-aware HPJA

2. **Regime-aware HPJA**
attempt with regime features (trend, volatility, volume, macro markers).
   - Estimate conditional distributions `P(outcome | price_in_zone_Z, regime, config)`.

3. **Parameter surface exploration**
   - Sweep `SL_pct`, `T_h`, and possibly multiple targets.
   - Map EVA and outcome mix over this grid for each zone.

4. **Zone-construction feedback loop**
   - Use HPJA statistics (success rates, EVA, drawdowns) as a scoring signal to refine or re-weight zones.

All of these build on the same core idea: **condition on a clearly defined price event and replay what history did after that event under consistent trading rules.**

---

## 10. TL;DR for Humans and AIs

- We are **not** modelling: "From today’s price, will we reach this zone and then hit target?"
- We **are** modelling: "Historically, whenever price was in this zone and we traded it with these rules, what usually happened?"
- An **attempt** is a single historical hypothetical trade starting from a candle where price was in the zone.
- The engine is an **event-based, conditional backtest** rooted in standard event-study and hitting-time ideas, and it is a sensible foundation to extend rather than throw away.
