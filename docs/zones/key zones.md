
# API Stats (no idea how often these update rn)
Find out API stats..... maybe updated once an hour or less... dunno, but I must have used some resources from somewhere today. I did about 20 requests I think. Count them..
    - 95,000 tokens in total.  Cost estimate: $0.25
    - Is that token limit per Month ? nope lol. Tokens Per Minute. Calm down.
    - looks like 11 questions I asked for project-b68cd74. That's pretty efficient coding.
    - Probably a few more for project-3402007.md because I was being a bit more lax. but still. did 2 big projects in a day. Don't ever really need more than that???

# Summary of what we need to work on regarding `target zones` :


# Add candlesticks
But too scared to do with Gemini 3 Preview (Remote edition) - much too fucking painful.

# 0.35
See when 0.35 version is due out and what features it will offer. Might help guide decision making
Appears like quite a big API change (sigh)
https://github.com/emilk/egui_plot/issues/200

## (Journey stuff) Sticky zone price target
Shoud price target be center rather than nearest edge of sticky zone ? (seems more natural than aiming for edge of structure, right?)
That's journey stuff I think, though, don't want let Gemimi loose on that yet, though, lol

# Target zone should be represented with a circle (the sniper target zone)
Cooolllll!!!!!!!!!!!!!!!!!!

# Notes
Weird thing when you get inside a Low Wick area, ie using Sim to move price up, it splits into Low Wick and High Wick Area. ie two 'interfering' triangles.
Why does live price make a difference here? Oh yes, of course, because price defines what is low and high wick zoens. So they will change based on price.
Seems fine then.

# Play with time_decay_factor
Try it at 2.0
default_time_decay_factor() in app.rs
How does it affect BTC / SOL etc.


# WebGL isn't supported in WASM Version
What is that all about? Has it stopped running for some reason?

# Never looked at this yet
            .view_aspect(PLOT_CONFIG.plot_aspect_ratio)
I think it should adjust to the space available rather than being fixed 2:1 or whatever? I thought it went wrong when the user readjusts the app window size.....