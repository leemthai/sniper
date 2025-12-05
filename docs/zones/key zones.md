
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


# Look again at this stuff tonight....
I really want to drill down into exactly where we see reversal zones, what the granularity is, do our reversal zones then tally with known reversal zones I will find on other trading software.
To that end:
1) I want the plot bar labels to print the $ range  the bar represents, something like this: "OLD TEXT ($1832.20 - $1855.40)"
How to test eliminating island gapping for reversal zones, for testing at least. So here are 3 parameters we have for reversal zones.
        let reversal_threshold = 0.0004;
         smooth_pct, 0.005,
         gap_pct   0.005,
Please remind me what all 3 achieve (couple of lines of text each explanation), and changing which would most help me zoom in on identifying only the most popular reversal zones.


# Reversal plots What we haven't done yet is fix tallest dwarf effect (but don't know how to, yet)
Reversal Zones: This creates the "Tallest Dwarf" visual effect. The gray bars will stretch to the top even if the zone is weak.
Recommendation: Leave it as normalize_max for now. This ensures the user can see where the wicks are, even if they are weak. The Colored Overlay (Target Zone) will only appear if the absolute score crosses the Global Threshold. This distinction (Visible Gray Bar vs. Missing Colored Zone) accurately communicates: "There is activity here, but it wasn't strong enough to be a target."

Find a pair near a reversal zone in Trading View and see how it looks in mine
Or just find one in trading view randomly that is up at resistance then can load into mine.
Reversal zones sshould be narrower. How can I achieve that?
Does bridging gaps even make sense for reversal zones? Not sure.
I want to see absolute numbers. How many wicks? Maybe a minimum % qualifying threshold?
So maybe show abs % numbers in plot for reversal zones, not normalized.

The size of reversal zones does actually vary between small sharp zones, and conglomerated zones. This might be ok. actually.
# "B"ackground key
"B" key currently to toggel round background plot type. See if reverals anything, particularly with reversal zones.
Note this is not gated for debug, it works for anyone
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
