
# API Stats (no idea how often these update rn)
Find out API stats..... maybe updated once an hour or less... dunno, but I must have used some resources from somewhere today. I did about 20 requests I think. Count them..
    - 95,000 tokens in total.  Cost estimate: $0.25
    - Is that token limit per Month ? nope lol. Tokens Per Minute. Calm down.
    - looks like 11 questions I asked for project-b68cd74. That's pretty efficient coding.
    - Probably a few more for project-3402007.md because I was being a bit more lax. but still. did 2 big projects in a day. Don't ever really need more than that???


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

## Sticky zone price target
Shoud price target be center rather than nearest edge of sticky zone ?

# 💡 Testing tip
Always be scaling price_horizon up and down. Very good way to get many different views of what this new algo does.
Do we end up with more and more output zones as we scale up the number of input zones?
    If so, it's not scale-independent is it.

# Low wicks and High wicks vs Reversal Zones
How are low wicks different from reversal zones? How to transform current low wick calcs into full reversal zones?
I could turn their graphs on now to see where they are of course
## Implement zone reversals `QRZ`
    - Hopefully be much simpler than `QSZ` algo because `QRZ` are naturally gonna be narrower zones right? maybe just a group of single zones? Let's see how it goes..... ask AI, lol
    - Validation: any way to confirm what we decide are `QRZ`, really are high probability reversal zones.





# Support / Resistance Zones
There is just one of each.
These are just `symbolic` zones i.e. `support zone` is nearest sticky zone beneath live price. `Resistance zone` is nearest sticky zone above live price.
Usage: Unknown (help user I suppose).


# Plot Presentation: How to visualize the combination of `QSZ` and `QRZ` for the user
in a way that is useful to them and easy to understand.
What to show on plot?
What should background bars be? And are they just debug? No, they feel integral
Can the use view both reversal ones and sticky zones at same time? Yes!
But if we do thst how do we choose which of the to the background bars are ?
Currently we can display both all sticky zones, plus the two (max) SR zones. So confusing already ? No, they should just be one thing though. Not separable.
## Legend
Can legend group bars of same type?



## WASM version (via trunk serve locally or web version): why stuck on 100 zones?
7. Why is this not up to date with latest zoning code?
    https://leemthai.github.io/zone-sniper/
    - How is trunk serve version?
    - Might be using 100 zones not 200 because we can't read from a state file.? Where do we set default? Need think about differences. Maybe I have set it to 200 now in config/analysis.rs
    - trunk serve version still feels like 100, however. what about web version? 100 as well. Oh well, lol. Maybe delete local state file and try again?
