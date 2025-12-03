
# API Stats
Find out API stats..... maybe updated once an hour or less... dunno, but I must have used some resources from somewhere today. I did about 20 requests I think. Count them..
    - 95,000 tokens in total.  Cost estimate: $0.25
    - Is that token limit per Month ? Oh shit...... nope. Tokens Per Minute
    - looks like 11 questions I asked. That's pretty efficient coding.


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


## QSZ work to do still....
1. Its good. Alot of old code to delete in zone_scoring.rs
2. Alot of code in trading_view.rs to create all those zones we don't use rn. They should be deleted (SR Zones, slippy zones etc. but keep the lowwick and highwick)
3. Try zone_scoring code with a variety of zones. Maybe increase resolution will actually help now. How to change? In state file?
yes. I tried 500 and seem to get lots of very very thin zones. So maybe not scale-independent? 200 is good number to build a good model on.
4. Tweak parameters at 200, see if happy with everything. If not, what don't I like. See if he can tweak that squaring algo he used ..... that was good.
5. see if any more functions are unneeded eg:
find_high_activity_zones () etc. -
find_support_resistance_superzones() etc.
6. Ensure I have his code e.g. find_target_zones() is exactly correct.
7. Find out exactly what we use: support_superzones() for. Definitely something ... maybe.


## Implement zone reversals `QRZ`
    - Hopefully be much simpler than `QSZ` algo because `QRZ` are naturally gonna be narrower zones right? maybe just a group of single zones? Let's see how it goes.....
    - Validation: any way to confirm what we decide are `QRZ`, really are high probability reversal zones.

## Presentation: How to visulaize the combination of `QSZ` and `QRZ` for the user
    in a way that is useful to them and easy to understand.

