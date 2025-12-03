
# Summary of what we need to work on regarding `target zones` (previously known as `key zones`):


## Rename `key zones` to `target zones`
    Currently, we use the term `key zones` in docs + probably code as well.
    I want to rename `key zones` to `target zones` (`key zones` is imprecise term)

## Incorporate `qualifying reversal zones` (`QRZ`) into `target zones` list
- Currently, the only zones that qualify as `target zones` are `qualifying sticky zones` (`QST`) (using very imperfect algos in `docs/zones/zone_scoring.md`)
- I would like to add `qualifying reversal zones` as `target zones` as well (see 2.4 of "System outline" section)
- Q: What happens during `merge` of `QRZ` and `QSV`? There are possbilities of overlaps etc. I guess we keep them as separate sub-lists. as we need to know whether the target zone is a reversal zone or sticky zone.


## Specify how `sticky zones` + `reversal zones` are calculated exactly
    - (I believe) current sticky zone algo is volume weighted body candles that intersect our price range, right?
    - What is justification for this?
    - Is there a better algo to generate sticky zones.
    - Why use body candles?
    - Discuss whether this is the right data for finding where price action spends the most time. How to know?
    - What is reversal zone calc? Is that volume weighted as well?


### Body candle vs high low.which to use?
Maybe more reasonable to use L to H rather than current O to C body candle? Yes. Coz Body is just about start and finish. Not about what happened in the day. Also, crypto stuff doesn't really have start of day and end of day. Yes. So kinda usless to just use body candle.
### Conclusion
if we do Time based then we have exact measure of time a pair has spent in price range
To increase accuracy we keep reducing the interval size from 1h to 30m to 5m etc
Then only question is volume weight or not? What value to assign a day where 1M shares were traded in a zone vs 1 share?
Definitely more valuable right!
Units would then change from minutes to minutes x trades ie TradeVolumeMinutes
Have weighting at this level as well, though. TemporalWeighting
Plus VolumeWeighting......
So we alreay scale by both QuoteVolume and TemporalWeighting ....... so do we do the 0 to 1 thing for the interval. What % of interval was spent in this zone.
I want to use full candle not body candle when calculating sticky zones. 


## AI Question 1
`src/journeys/decay_calibration.rs`
`src/journeys/zone_efficacy.rs`
Please explain, in a couple of paragraphs, what these two files are about, and do we currently utilize their results in the app at all?
## AI Answer 1:
Here is the breakdown of those two files and their current utilization:

**What they are about:**
*   **`src/journeys/zone_efficacy.rs`** calculates quality metrics for your "Sticky Zones." It analyzes historical price action to determine how "effective" the current zones are by computing statistics like **Dwell Duration** (how long price sits inside a zone before leaving) and **Occupancy** (what percentage of price action happens inside zones vs. outside). It essentially answers the question: "Are these zones actually sticky?"
*   **`src/journeys/decay_calibration.rs`** is an optimization engine designed to "tune" the CVA algorithm. It is meant to run multiple CVA simulations with different `time_decay_factor` settings, score them using the metrics from `zone_efficacy.rs`, and automatically pick the decay factor that produces the "stickiest" zones.

**Do we utilize them?**
*   **Zone Efficacy:** **Partially.** The function `compute_zone_efficacy` is actively called in `app_state.rs` whenever CVA results are updated. However, the results are stored in `self.data_state.zone_efficacy` and are **only displayed** in the UI status bar if the developer flag `print_sticky_dwell_summary` is enabled. For a standard user, this calculation runs but is invisible.
*   **Decay Calibration:** **No.** This appears to be dormant/dead code. The main application loop (`app_async.rs`) uses the specific `time_decay_factor` from your settings and never calls `calibrate_time_decay`. The system currently relies on manual configuration rather than this auto-tuning logic.

# AI Question 2
Sticky zones are currently calculated by this process:
    timeseries.rs -> process_candle_scores() -> increase_score_multi_zones_spread()
What I want to achieve is just measure the zones where price action spends the most time. And then *possibly* weigh that by (a) temporal weighting (to increase import of recent data) and (b) Volume weighting (to increase the import of heavily traded time periods) 
Don't forget the app has the intention of using sticky zones as *price targets*. The idea being, once the price reacjes this sticky zone, it is an opportune time to end the trade, because historically trading has consolidated at this particular price level.
1. Can we learn any lessons from the basically unused zone_efficacy.rs code and incorporate it into the sticky zone calculation?
2. Why don't we use the full candle not just the body candle? Crypto is traded 24/7 everywhere. I don't see that open/close for a candle is important at all really. The full candle is providing us more information than just the body candle I believe? So currently we use this code I believe:
        let sticky_data = maths_utils::normalize_max(cva.get_scores_ref(ScoreType::CandleBodyVW));
I believe we should instead use the full candle, not just the body for that calculation. (The full candle is not a ScoreType yet I believe, but we could certainly add it...)
3. I really don't know how to go about using both the temporal weighting and volume weighting. I mean, we could do the sticky zone calculations as bare-bones *no weighting*, then would it be purely be just "the price levels where the price action has spent the most time"?
4. Soon (after finalizing these sticky zone questions) I want to add 'reversal zones' to the system, because obviously, reversal zones, not just sticky zones, are natural price targets as well. These are calculated using LowWick (if price target is down) + HighWick (if price target is up) data. So please bear this upcoming expansion in mind, as I'm sure many of the same questions will apply to reversal zones.
In summary: I just want "sticky zones to be reliably sticky" lol. So the user and system trusts them as good price targets.. so how to improve the current sticky zone calculations so they function well as predictive sticky zones. Plus, I want to dump the *separate* code in these files:
`src/journeys/decay_calibration.rs`
`src/journeys/zone_efficacy.rs`
and incorporate any good ideas from them into the actual sticky zone calculation code.
# AI Answer 2
Here is a breakdown of how to restructure your Sticky Zone logic to be robust, simpler, and aligned with your goal of using them as price targets.

### 1. Lessons from `zone_efficacy.rs` (The "Density" Lesson)
The core value of `zone_efficacy.rs` was that it tried to measure "Dwell" (how long price stays in a zone).
**The Lesson:** You don't need a separate complex file to measure this *after* the fact. You can bake "Dwell" directly into the scoring mathematics.

If a candle represents 1 hour of time:
*   **Big Candle (High Volatility):** Price moved through many zones. It spent very little time in any specific zone. (Low Dwell Density).
*   **Small Candle (Consolidation):** Price stayed within a narrow range. It spent the entire hour in just one or two zones. (High Dwell Density).

**The Solution:** Instead of adding complex post-processing, we simply change the math in `timeseries.rs`. When we apply a score to a set of zones, we **divide the score by the height of the candle**. This naturally makes short candles (consolidation) build massive scores in specific zones, while tall candles (sprints) dilute their score across many zones.

### 2. Full Candle vs. Body Candle
**You are 100% correct.** In 24/7 crypto markets, Open and Close are arbitrary snapshots.
*   **Current Logic:** Uses `Open` to `Close`. If a candle wick goes to a specific level and trades 1,000 BTC there, but closes back down, your current logic ignores that volume completely.
*   **New Logic:** Use `Low` to `High`. This represents the entire **Price Opportunity** range where trading actually occurred.

### 3. How to combine Temporal and Volume Weighting
You are overthinking the complexity here—it fits into a single elegant formula. You essentially want to create a **Volume-Time Density** map.

Here is the "Golden Formula" for your `increase_score_multi_zones_spread` call:

$$ \text{Total Weight} = \text{Volume} \times \text{TemporalDecay} $$

Then, you spread this weight across the zones covered by the candle (`High` to `Low`).
*   **If Volume is high:** The zone gets a higher score (Strong agreement on price).
*   **If Candle is recent:** The zone gets a higher score (Relevant agreement).
*   **If Candle is short (Consolidation):** The weight is concentrated into 1 or 2 zones, creating a "Peak" (Sticky Zone).
*   **If Candle is tall (Expansion):** The weight is spread thin over 20 zones, creating a flat area (Slippy Zone).

### 4. Implementation Plan

You can safely delete `src/journeys/decay_calibration.rs` and `src/journeys/zone_efficacy.rs`. Their logic is being moved "upstream" into the generation phase.

Here is exactly how to modify your code to achieve this.

**A. Update `src/models/cva.rs`**
Add a new ScoreType (or rename the old one) to reflect that this is now Full Candle analysis.

```rust
pub enum ScoreType {
    // Rename CandleBodyVW to this:
    FullCandleTVW, // Time-Volume-Weighted Full Candle
    // ... keep wicks for your future Reversal Zone work
}
```

**B. Update `src/models/timeseries.rs`**
Modify `process_candle_scores` to implementation the Density Logic.

```rust
fn process_candle_scores(&self, cva_core: &mut CVACore, candle: &Candle, temporal_weight: f64) {
    let (price_min, price_max) = cva_core.price_range.min_max();
    
    // Helper to clamp to analysis range
    let clamp = |price: f64| price.max(price_min).min(price_max);

    // 1. FULL CANDLE ANALYSIS (The new Sticky Logic)
    // We use Low/High instead of Open/Close
    let candle_low = clamp(candle.low_price);
    let candle_high = clamp(candle.high_price);
    
    // We use Base Asset Volume (amount of crypto traded) 
    // multiplied by your temporal decay.
    // NOTE: You could add 1.0 to volume if you want a minimum "Time" score 
    // even on 0 volume candles, but volume usually suffices.
    let weight = candle.base_asset_volume * temporal_weight;

    // This function inside CVA needs to automatically divide 
    // the weight by the number of zones covered. 
    // If your `increase_score_multi_zones_spread` already does that 
    // (divides weight by n_chunks), you are good. 
    // If it adds the full weight to every zone, you need to divide it here.
    cva_core.increase_score_multi_zones_spread(
        ScoreType::FullCandleTVW,
        candle_low,
        candle_high,
        weight 
    );

    // ... Keep your wick logic for step 4 (Reversal Zones) ...
}
```

### Summary
By switching to **Full Candle** and ensuring the weight is **distributed** (divided by the height of the candle in zones), you mathematically guarantee that:
1.  Areas where price chopped sideways for days (short candles, high volume) become massive peaks (**Sticky**).
2.  Areas where price flew through in minutes (tall candles) become low valleys (**Slippy**).
3.  Reversal zones (Step 4) will naturally complement this because they will look for the *edges* (Wicks) of these ranges, whereas this logic finds the *center of gravity*.

# AI Question 3
just a quick question: in cva.rs increase_score_multiz_zones_spread() we already have this line:
        let quantity_per_zone = score_to_spread / (num_chunks as f64);
Isn't that supposed to do what you mentioned i.e. "When we apply ascore to a set of zones, we divide the score by the height of the candle"?

# AI Answer 3
**Yes, exactly.**

That specific line of code (`score_to_spread / num_chunks`) is the "magic sauce" that creates the density map I described.

Because that line exists, the logic is already doing the heavy lifting for you:
1.  **Short Candle (Consolidation):** `num_chunks` is small (e.g., 1 or 2). The volume is divided by a small number, resulting in a **High Score** added to those specific zones.
2.  **Long Candle (Volatility):** `num_chunks` is large (e.g., 20). The same volume is divided by 20, resulting in a **Low Score** added to many zones (diluting the impact).

### What this means for your implementation:
You **do not** need to write complex new math. The engine is ready. You simply need to feed it the correct data in `src/models/timeseries.rs`.

**The only change you need to make to achieve your goal:**
Stop feeding it `Body` (Open/Close) and start feeding it `Full Candle` (Low/High).

In `src/models/timeseries.rs`, inside `process_candle_scores`:

```rust
// OLD WAY (Body Only):
// let body_start = clamp(candle.open_price.min(candle.close_price));
// let body_end = clamp(candle.open_price.max(candle.close_price));

// NEW WAY (Full Candle - capturing all price action):
let candle_low = clamp(candle.low_price);
let candle_high = clamp(candle.high_price);

// Calculate the Total Weight (Volume * Time Decay)
// We rely on 'increase_score_multi_zones_spread' to divide this by the height (density).
let total_weight = candle.base_asset_volume * temporal_weight; 

cva_core.increase_score_multi_zones_spread(
    ScoreType::FullCandleTVW, // Rename CandleBodyVW to this
    candle_low,
    candle_high,
    total_weight
);
```

This simple change leverages the existing math to automatically highlight areas where price consolidated (short candles) with high volume, effectively replacing the need for the external logic in `zone_efficacy.rs`.

# AI Question 4
just a quick question: in cva.rs increase_score_multiz_zones_spread() we already have this line:
        let quantity_per_zone = score_to_spread / (num_chunks as f64);
Isn't that supposed to do what you mentioned i.e. "When we apply ascore to a set of zones, we divide the score by the height of the candle"?

# AI Question 5



















## Analyze distribution/normalization of source zone data

Q. Do I still want to analyse distribution/normalization of source zone data? 
    - yes. but maybe lower priority now.
    - what I really want to do is turn each zone score into HRS/MINS/SECS. Make it meaningful both in the app and to the user.
- It's just an unbounded range of positive numbers scaled down by dividing by the largest number in the set. So max_value in scaled set is ALWAYS 1. min_value in scaled set is >= 0.
- So the distribution of normalized data depends directly on distribution on non-normalized data.
- Great Example: `ETHUSDT` - I'm sure the average score here is very, very high. Yes, this is because the highest score is not that much bigger than the median score, and this probably indicates low (volume) volatilty in the trading pair, right?
- Find example of opposite: `PAXGUSDT` - I'm sure the average score here is very very low. Indicating massive (volume) volatility spike at the key levels.
- What does this all mean? Something like (1) pairs with higher volume volatility  will have much more pronounced `QSZ` (depending on algo). And pairs with lower volume volatility will produce zone scores that are very low SD, all *much of a muchnes* - so what does that all mean for the algo ??? And what type of algo would work? Does it mean we are normalizing wrong?
- How to analzye? Just find code that could output debug the SD/variance of the zone scores for 100 zones, ETHUSDT vs PAXGUSDT. Hope they are very far apart and go from there.
- Will the typical distribution of this data affect the `QSZ` algo? Yes for sure, the current one anyway. But maybe it is supposed to..... because the current algo relies on volume volatility, right? So is volume is not volatile, it is naturally hard or impossible to define `sticky zones` by the variability in weighted volume.
- I can add pics of `PAXGUSDT` and `ETHUSDT` (or `BNBUSDT`) as evidence.


## Improve our algorithm to find `QSZ`
- What is wrong with current algo?
    - It works well for some pairs, less well for others
    - It works well for some zones, less well for others
    - I haven't yet specified what I want to be a `QSZ`. So how can it work well across the board?
    - Is it worth analyzing/working on current algos or better just to describe the API I require and let various AI suggestions t try and find a better algo? (Not sure yet.)
    - Can a single zone really be described a a `target sticky zone`? In other words, the nub of the question here is: all other aspects being identical, is it that the total volume across the zone is the most important overriding principle in marking something as qualifying? 
        - Example: a single zone with very high volume, but big drop-off on either side. So there is very little chance of the price staying within this high volume zone. It's just one-pip wide after all.
        - We are looking for broad action across a number of zones. Like a traffic jam area really of high traffic. And that obviously ends once the traffic dies off.
        - Feels to me like less of a `find peak` algo. more of a `find high traffic areas` with `low traffic areas` on either side.
            - the devil is in the detail there though of course. What if we get a lovely high traffic area across 4 zones, but then a gradual gradient away to lower traffic areas. Or even, gradual gradient away for a bit, but then up to high traffic areas again. There is no reason with our data to get this pattern: `find high traffic areas` with `low traffic areas` on either side.
    - Decide whether the approach vector really is important or not in defining `QSZ`? if so, then don't we have a dynamic system? Or at least a system which depends on direction of travel? Answer with this experiment:
        - Look at a graph
        - decide what is a reasonable `QSZ`
        - decide whether approaching this zone from above or below makes any difference to it being *reasonable* still. For now, I don't think approach vector is important. 
    - Boundary cases in current version only sort of work (but only with bodgy after-thought type code e.g. zone 0, zone `max_zones`-1. This needs improving.)
- What ideas we bring to make it work better?
    - Idea: Find major peaks, plus minor peaks (sufficiently far from major peaks?), plus major sustained levels
    - Zone Image analysis via AI? Probably several ways for this e.g. feeding it images(of what?) etc. (but that could be a whole project in itself so leave it for now)
    - Specify that the output number of zones is variable not fixed (justification: we do not know in advance how many target zones will be available for a given map)
- (Wish list for new version) - That the new algo works well (and as far as possible, identically) regardless of how many zones we have: 100 or 400 or 1000 etc. Then we can find a more optimal zone_count number (currently fixed at 100 just to keep my sanity)
- That it is fully plug-n-play via a friendly API-type system:
    - Define the exact inputs
    - Define the exact outputs
    - Then when we plug-in any-old AI-generated code to see how it does.


## Create algorithm for finding `QRZ`
    - Hopefully be much simpler than `QSZ` algo because `QRZ` are naturally gonna be narrower zones right? maybe just a group of single zones? Let's see how it goes.....
    - Validation: any way to confirm what we decide are `QRZ`, really are high probability reversal zones.

## Presentation: How to visulaize the combination of `QSZ` and `QRZ` for the user
    in a way that is useful to them and easy to understand.

