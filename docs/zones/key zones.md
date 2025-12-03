
# Summary of what we need to work on regarding `target zones` :

## Rename `key zones` to `target zones`
    Currently, we use the term `key zones` in docs + probably code as well.
    (previously known as `key zones` conceptually at least, maybe not in app yet.)
    I want to rename `key zones` to `target zones` (`key zones` is imprecise term)
    Need update spec.md as bare minimum.

## Incorporate `qualifying reversal zones` (`QRZ`) into `target zones` list
- Currently, the only zones that qualify as `target zones` are `qualifying sticky zones` (`QST`) (using very imperfect algos in `docs/zones/zone_scoring.md`)
- I would like to add `qualifying reversal zones` as `target zones` as well (see 2.4 of "System outline" section)
- Q: What happens during `merge` of `QRZ` and `QSV`? There are possbilities of overlaps etc. I guess we keep them as separate sub-lists. as we need to know whether the target zone is a reversal zone or sticky zone.


# Next
Go over new results. How does it feel? How are distributions of new sticky zones?
Temporaral weight is, I believe. 1.0. So effectively null. So the old question reamins - what value SHOULD it be?
    - how can we tell?
    - does it matter whether the temporal range we are dealing with is small (hours) vs days or weeks or months??????
Get on with QRZ
Find out API stats..... maybe updated once an hour or less... dunno, but I must have used some resources from somewhere today. I did about 20 requests I think. Count them..
    - 95,000 tokens in total.  Cost estimate: $0.25
    - Is that token limit per Month ? Oh shit...... nope. Tokens Per Minute
    - looks like 11 questions I asked. That's pretty efficient coding.

AI said:
Summary (of `restructure your Sticky Zone logic to be robust, simpler, and aligned with your goal of using them as price targets.`)
By switching to Full Candle and ensuring the weight is distributed (divided by the height of the candle in zones), you mathematically guarantee that:
- Areas where price chopped sideways for days (short candles, high volume) become massive peaks (Sticky).
- Areas where price flew through in minutes (tall candles) become low valleys (Slippy).
Reversal zones (Step 4) will naturally complement this because they will look for the edges (Wicks) of these ranges, whereas this logic finds the center of gravity.

My question for later: 
"whereas this logic finds the center of gravity." - how does this play into superzone formation etc?

Q: do we display logging errors in release version? great question.
A: Dunno. But maybe we should?
But also make sure we have error vs warn sorted as well
log::error!
log::warn!
I guess log doesn't load at all in release version... so no, it's debug-only systm.


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

