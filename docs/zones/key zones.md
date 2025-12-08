
# API Stats (no idea how often these update rn)
Find out API stats..... maybe updated once an hour or less... dunno, but I must have used some resources from somewhere today. I did about 20 requests I think. Count them..
    - 95,000 tokens in total.  Cost estimate: $0.25
    - Is that token limit per Month ? nope lol. Tokens Per Minute. Calm down.
    - looks like 11 questions I asked for project-b68cd74. That's pretty efficient coding.
    - Probably a few more for project-3402007.md because I was being a bit more lax. but still. did 2 big projects in a day. Don't ever really need more than that???

# Summary of what we need to work on regarding `target zones` :


# Bugs in Plot stuff
    - Left with 'null window' hanging around because an egui_plot must have an 'over hover label' - no way to turn it off it seems rn. All the hover text is a mess rn. Just leave it

# B key to select background bars
Much too manual job at the moment
Should be tied to which zones we are currently viewing
But if we do that, can't view all zones on same map. Not good.
"B" key currently to toggel round background plot type. See if reverals anything, particularly with reversal zones.

# Reversal Zones quite often appear within bounds of High Volume Zones
Looks quite nice, maybe just tweak the colors?
Might be quite nice to make reversal zones much more 'different than just single colours
Add some arrows pointing in the reversal direction perhaps? Interesting... Maybe not even fill the zone with color, just decorate with upward or downward arrows drawn with 3 lines each?
What is a good image or animation to represent "reversal up" or "reversal down" ....
But it has to work whether zone is whatever width (often a pixel or two wide if single zone....)

# Still not sure whether having orange for ALL active zones is good idea
coz then it's not obvious whether we are in sticky zone rn (which is obviously the most likely by far)
or the much rarer "in reversal zone"
Must be a better solution than making them all identcal colour.
If reversal zones had a different 'look' or pattern from sticky zones, then colour could indeed be orange for both...

# Not good that when you turn off a certain zone in legend, you switch to new pair and it turns on again...


Implementing now...
Jump keys: 1/2/3 - these don't have have keyboard prints yet at all. I forgot about them actually. Need to print them.
There is no printing to say what 1/2/3 do ......
Plus need test Shift+1, Shift+2, Shift+3 funciton in sim mode. Make sure they don NOT work in non-sim mode, too.
Plus change "Cuts" in help window, coz weird.
View: text is wrong - should be sticky, low wicks, high wicks


# Add candlesticks
But too scared to do with Gemini 3 Preview (Remote edition) - much too fucking painful.

# 0.35
See when 0.35 version is due out and what features it will offer. Might help guide decision making
Appears like quite a big API change (sigh)
https://github.com/emilk/egui_plot/issues/200

## (Journey stuff) Sticky zone price target
Shoud price target be center rather than nearest edge of sticky zone ? (seems more natural than aiming for edge of structure, right?)
That's journey stuff I think, though, don't want let Gemimi loose on that yet, though, lol

# How are we going to do more with this model?
Don't want it attacking big rebuild projects
Can I try Zen again?
Will certainly have mature github project to try and login with ....

# WebGL isn't supported in WASM Version
What is that all about? Has it stopped running for some reason?