# Root Specification Document for `Zone Sniper Project`

## A full glossary of terms
See `docs/glossary.md`

## Technical Requirements for AI coding
See `docs/technical/ai_technical.md`

## Introduction:
Find high probability trading opportunities NOW for any asset given the current price and market conditions. In particular, we highlight nearby `key trading zones` and calculate the likelihood of reaching each one from current price (within user's preferred time frame).
  
## System outline
1. Import historical price action (klines) for all live system pairs.
2. Create `key price zones` for all pairs with sufficient kline data for the user's preferred `price horizon`. What is the validation for selecting `key price zones` as price targets?
    - 2.1 Pairs spend a disproportionally large amount of time in `key price zones`
    - 2.2 Therefore once an asset reaches a `key price zone` from outside such a zone, it is then likely to consolidate sideways for a while. Therefore profit should be taken and the capital re-invested in other opportunties.
    - 2.3 Therefore `key zones` make statistically valid `price targets` (as are `reversal zones`)
    - 2.5 (Not coded at all yet) how does the presence of `reversal zones` afffect journey outcomes to `key price zones` i.e. what if this reversal zone is found between `live price` and a `key price zone` - does this:
        - 2.5.1 invalidate the `key price zone` target?
        - 2.5.2 modify the probability of `key zone` being reached?
3. For all pairs, given its `live price`, for each `key price zone` run a plug-in analysis function to:
    - (this is journey analysis `docs/journeys/journey_spec.md`) find the trades with highest EVA. i.e. generate a list of potential trades and add these trades to a global list and present these opportunties to the user to be viewed, sorted, filtered, analyzed, selected.
    - Note this is currently completely bugged due to a misunderstanding ie. the AI coded journey analysis from `nearest key zone` to `other key zones` and I need `live price` to `all key zones` - so the journey work is on-hold for now.


## More on Journeys
(ON-HOLD) `docs/journeys/journey_spec.md` (plus all other docs in that folder - `docs/journeys/reconcile.md` in particular is very important)

## More on UI:
See `docs/ui/trade_ui.md`

## More on Web Demo:
See `docs/WASM/web_demo.md`

## More on Various Technical Elements
Interval Switching: `docs/technical/interval_switching.md`
AI Coding: `docs/technical/ai_technical.md`
Discontinuous Ranges: `docs/technical/discontinuous_ranges.md`
