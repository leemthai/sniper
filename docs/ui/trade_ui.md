# Journey UI Control Discussion

_Last updated: 2025-11-18_

This note captures the current thinking around the UI controls that let users shape the trade journeys they want to explore.

For details on the underlying journey engine and how these controls feed analysis, see `docs/journeys/journey_spec.md` in this directory.

## 1. Trade Parameters (“Shape your trading opportunities”)

### 1.1 Price Horizon
- **What it does**: Acts as a price-axis zoom centred on the live price. Tight ranges emphasise local structure; wide ranges surface broader swings
- **Intended message**: “Zoom into local price action” ←→ “Zoom out to the wider price panorama.” Keeps attention on price proximity rather than recent time.
- **Zone behaviour**: Always yields 100 sticky zones. Scores are normalised against the top zone inside the selected band, so even quiet areas produce meaningful rankings.
- **Data sourcing**: The system gathers every historical candle whose body intersects the band. Samples are non-contiguous in time; users shouldn’t equate narrow bands with “recent only.”
- **Interpretation pitfalls**: Users might *assume* a smaller band means less data, but the non-contiguous sampling still captures rich history. Clear tooltip copy should reinforce that this is a price zoom, not a timeframe selector.
- **Adaptive resolution idea**: Dynamically adjust candle interval (higher frequency when zoomed in, lower when zoomed out) to maintain robust sample sizes.

### 1.2 Time Horizon
- **What it does**: Caps how long a journey is allowed to reach its target before we treat it as a timeout.
- **Intended message**: “Set time limit on trades you care about.”
- **Why it matters**: Encodes the trader’s patience/urgency (“I need this setup to resolve within X days”).
- **Challenges**: Less intuitive than price range—requires translating a trading cadence into a numeric duration. Without guidance, some users might ignore it or set unrealistic windows.
- **System impact**: Changing this parameter forces HPJA to recompute outcomes for every pair, since it alters the success/failure boundary.
- **Relationship to price range**: Feels tightly coupled conceptually—users may think of both as “trade style” controls. For now it remains an analysis input rather than a display-only filter.

### 1.3 Tensions Between Price & Time Horizons
- **Trade-off**: Wide Price Horizons paired with tight Time Horizons effectively ask for large-distance moves that finish fast. Those do occur but are rare, so HPJA will often return mostly timeouts or stop-outs.
- **Feedback**: Consider surfacing aggregate stats (e.g., “Only 2% of journeys hit target within 2 days at this distance”) to educate users when their settings collide.
- **Guidance**: inline hints (“Very short times on wide ranges may yield few candidates”) can steer exploration without blocking advanced users.
- **Future ideas**: Auto-suggest relaxing one control when the other tightens too far, or highlight historical examples that did meet the aggressive criteria so traders understand the edge cases.

For specific orchestration triggers that determine when journey results are recomputed after these controls change, refer to section 6 of `docs/journeys/journey_spec.md`.

## 2. Guidance / UX Ideas

1. **Future controls to consider**
   - **Risk appetite**: Direct stop-loss % tweak or a low/medium/high selector that maps to stop-loss presets.
   - **Direction filter**: Allow users to quickly flip between long-only, short-only, or both.
   - **Zone filters**: Let users specify the type of sticky zone (fresh vs. retest) or strength thresholds once that data is available.
   - **Outcome emphasis**: Toggle to highlight high-probability vs. high-reward journeys in the results view.

## 3. Next Steps
- Gather UI feedback on how often the price-range slider is adjusted versus other controls to prioritise future work.
- Revisit this document once additional controls are introduced or user testing uncovers friction.

## 3. Plot Considerations
- The current chart plots zone strength along the horizontal axis but still labels it with dates, which is misleading. We need to relabel the x-axis (and supporting annotations) to emphasise “zone strength” or similar price-structure metrics.
- Future enhancement: visualise the non-contiguous candle ranges that contributed to each sticky zone so users can relate zoom settings to historical coverage.
