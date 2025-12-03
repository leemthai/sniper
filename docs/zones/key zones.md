
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


## Validation: What I really don't understand yet is how (if possible) to confirm that:
- what we decide are `QSZ`, really are the most sticky zones available.
- And if we can validate zones with this `other method`, why don't we use this other method in the first place to establish the zones rather than going through all the pain of defining complex algos?
- It does make sense that heavy volume areas are areas where price spend more time.
- But if you want pure 'areas where prices spend lots of time' why not just count up the body candles that intersect this zone, rather than the volume-weighted version? So maybe the non-volume weighted version *IS* the validation we are seeking. Coz that literally is just counting up how much time a pair spends in a certain zone, right.
- Yes. And if we went to straight body candle count, that would remove all the SD problems rght? We can find out tmr.
    - We find SD with current zone methods.
    - Switch to non-weighted methods.
    - Find SDs
    - Compare them.
    - And make sure to check where the weighting factor kicks in. very important (in theory, but if set to 1, irrelevant for now)

## New Thoughts on Wednesday - Accurately Judging Time Spent In Zones
Why not use a value of 0 to 1 for each candle:

1 means spent all the time period within the price range
0 means soent no time in the candle thst day
So if interval is 5m and score is 1. Then we know the pair spent 5x1m = 5m in that price range
Then we just add the score up for each zone

Do we do that already for volume weighted calc? Need to analyse current code.
Maybe we do the opposite i.e. maybe we give a bigger score the bigger the candle is. i.e if candle is spread over several zones we increase the score in each zone. Not sure yet

### Body candle vs high low.which to use?
Maybe more reasonable to use L to H rather than current O to C body candle? Yes. Coz Body is just about start and finish. Not about what happened in the day. Also, crypto stuff doesn't really have start of day and end of day. Yes. So kinda usless.

### Conclusion
if we do Time based then we have exact measure of time a pair has spent in price range
To increase accuracy we keep reducing the interval size from 1h to 30m to 5m etc

Then only question is volume weight or not? What value to assign a day where 1M shares were traded in a zone vs 1 share?
Definitely more valuable right!

Units would then change from minutes to minutes x trades ie TradeVolumeMinutes


# Ask AI: Dwell
Is that code used slready to fimd out how long a price atays in a certain price range?
Get ai studio to explain it to me but can't give it context unless i send it entire codebse first ( I can do that)
So do that but need sort out docs as well
In context of `src/journeys/decay_calibration.rs`

# Wednesday - Make a plan
1. Get docs to a level I am happy commiting
    - Includes fixing up all references in docs to other docs or source code
    - Make them all relative to project root e.g:
        - src/config
        - docs/ (There is no docs/spec or spec/ now - just docs/)
            - docs/spec.md, docs/glossary/md, docs/journeys/, docs/ui/
        - Search for all references in docs folder for .rs, .md
2. Try out giving aistudio access to the codebase via https://repomix.com (will this include docs? if not, how to add?)
3. Ask AI questions I have, including:
    - Specify how `sticky zones` + `reversal zones` are calculated exactly (point it to exact functions)
    - At what stage is weighting applied
    - Explain Dwell code - what it does plus where it is used in the app
4. Do I still want to analyse distribution/normalization of source zone data? 
    - yes. but maybe lower priority now.
    - what I really want to do is turn each zone score into HRS/MINS/SECS. Make it meaningful both in the app and to the user.


## (To DO FIRST COZ THIS IS ROOT CAUSE AND I HAVE HOOK INTO IT NOW) - Analyze distribution/normalization of source zone data
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

## Q. What do `decay_calibration.rs`, `zone_efficacy.rs`, `dwell_durations` have to do with all this?
    - It *is* about analyzing sticky zone efficacy, so yes, this was designed I believe for the algo. We wanted to analyze what difference it made altering the weighting parameter. And AI came up with the idea of zone_efficacy.rs and that whole system....
    - At what stage do we analyse this weighting and introduce into the calculations?
        - We don't use it rn I believe. We just use a fixed weighting.
        - it is used in zone_efficacy.rs. But I don't really understand it. 