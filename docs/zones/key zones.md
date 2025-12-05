
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
That's journey stuff I think, though, don't want let Gemimi loose on that yet, though, lol

# 💡 Testing tip
Always be scaling price_horizon up and down. Very good way to get many different views of what this new algo does.
Do we end up with more and more output zones as we scale up the number of input zones?
    If so, it's not scale-independent is it.

# Reversal Zones Notes
Huge SOLUSDT reversal zone between $124 and $165 ish. Maybe this is just very revealing
The size of reversal zones does actually vary between small sharp zones, and conglomerated zones. This might be ok. actually.

# Very base dfn. of reversal zones
Something interesting about reversal zones. Scores very similar to sticky zones...... because all volume weighted right? Shouldn't reversal zones be pure counts, regardless of volume. Interesting thought antyway
Currently the volume scalar dwarfs the wick count I think. Why not just have wick count instead?
How to scale down the effect of the volume? Need an extra parameter? 

# "B"ackground key
"B" key currently to toggel round background plot type. See if reverals anything, particularly with reversal zones.
Note this is not gated for debug, it works for anyone

## Legend
Can legend group bars of same type?
That would be great.


## WASM version (via trunk serve locally or web version): why stuck on 100 zones?
(UPDATE) It DOES! have latst code (coz "B" key works to alter background). But still using a very old 100 zone count. Why?
- How is trunk serve version?
- Might be using 100 zones not 200 because we can't read from a state file.? Where do we set default? Need think about differences. Maybe I have set it to 200 now in config/analysis.rs
- trunk serve version still feels like 100, however. what about web version? 100 as well. Oh well, lol. Maybe delete local state file and try again?
