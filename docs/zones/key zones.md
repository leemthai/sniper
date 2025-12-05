
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

# 💡 Testing tip
Always be scaling price_horizon up and down. Very good way to get many different views of what this new algo does.
Do we end up with more and more output zones as we scale up the number of input zones?
    If so, it's not scale-independent is it.


## What to change to get more or less reversal zones:
Not sure good value for reversal_threshold. Depends how many reversal zones we want I guess.
# Try getting rid of islands ow as well
Done that. Still get lots of zones stuck together. SO maybe don't want islands at all for wicks.
# We now have temporal weighting in as well.
But this has not been tested really. Not sure how to test
# Tooltips are useless for wicks
Not sure would be useful to print, though. Maybe absolute numbers. How many wicks? Maybe a minimum % qualifying threshold?
So maybe show abs % numbers in plot for reversal zones, not normalized.
# Reversal plots What we haven't done yet is fix tallest dwarf effect (but don't know how to, yet)
(This is still true, right?) Reversal Zones: This creates the "Tallest Dwarf" visual effect. The background bars will stretch to the right even if the (strongest) zone is weak.
# "B"ackground key
"B" key currently to toggel round background plot type. See if reverals anything, particularly with reversal zones.
Note this is not gated for debug, it works for anyone
Is there a better way to trigger this change-  something more auto?
# Find cases where high/low wicks are significatly different than sticky zones
This is the real point of it after all
# Do low wick areas and high wick areas every vary much?
Maybe off into price discovery I would have thought
# What does it mean if live price is in both:
Active Resistance (wick), and
Active Support (wick)
at same time. Just overlapping zones?
# Decide on final reversal_threshold
current value: let reversal_threshold = 0.00001; // About 0.3% wick density 
        

## Legend
Can legend group bars of same type?
That would be great.
## Sticky zone price target
Shoud price target be center rather than nearest edge of sticky zone ? (seems more natural than aiming for edge of structure, right?)
That's journey stuff I think, though, don't want let Gemimi loose on that yet, though, lol





## WASM version (via trunk serve locally or web version): why stuck on 100 zones?
(UPDATE) It DOES! have latst code (coz "B" key works to alter background). But still using a very old 100 zone count. Why?
- How is trunk serve version?
- Might be using 100 zones not 200 because we can't read from a state file.? Where do we set default? Need think about differences. Maybe I have set it to 200 now in config/analysis.rs
- trunk serve version still feels like 100, however. what about web version? 100 as well. Oh well, lol. Maybe delete local state file and try again?
