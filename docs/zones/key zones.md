
# API Stats (no idea how often these update rn)
Find out API stats..... maybe updated once an hour or less... dunno, but I must have used some resources from somewhere today. I did about 20 requests I think. Count them..
    - 95,000 tokens in total.  Cost estimate: $0.25
    - Is that token limit per Month ? nope lol. Tokens Per Minute. Calm down.
    - looks like 11 questions I asked for project-b68cd74. That's pretty efficient coding.
    - Probably a few more for project-3402007.md because I was being a bit more lax. but still. did 2 big projects in a day. Don't ever really need more than that???

# Add candlesticks
Soon mate, Soon.

## (Journey stuff) Sticky zone price target
Shoud price target be center rather than nearest edge of sticky zone ? (seems more natural than aiming for edge of structure, right?)
That's journey stuff I think, though, don't want let Gemimi loose on that yet, though, lol

# Target zone should be represented with a circle (the sniper target zone)
Cooolllll!!!!!!!!!!!!!!!!!!

# Next 
Command stuff for keyboard action

# Stop printing hover windows in random colors
I want to print in fixed colors somehow

# Notes
Weird thing when you get inside a Low Wick area, ie using Sim to move price up, it splits into Low Wick and High Wick Area. ie two 'interfering' triangles.
Why does live price make a difference here? Oh yes, of course, because price defines what is low and high wick zoens. So they will change based on price.
Seems fine then.

# Things I can fix myself without AI
1.  Play with time_decay_factor
    - Could do this on my own without AI help......
    - Try it at 2.0. What other values. What does 2.0 do exactly? (Setting this to 2.0 activates "Annualized Decay" (Data today is 2x stronger)
    - default_time_decay_factor() in app.rs
    - How does it affect BTC / SOL etc.


# Notes: Don't forget any time we print prices, use format_price() instead of just ${:.2} or whatever.
Fixed via format_price()

# 0.35
See when 0.35 version is due out and what features it will offer. Might help guide decision making
Appears like quite a big API change (sigh)
https://github.com/emilk/egui_plot/issues/200



# Are we serializing too much? (vague guess)
Have a look at state file soon in my spare time
Get AI to have a look at state.json file. He can analyse

# Next big job - cloning stuff
When we reconvene, we will audit the `JobRequest` $\to$ `Worker` $\to$ `pair_analysis` pipeline to ensure we are passing **References** and **Arcs**, not deep copies.
Enjoy the rest. The foundation is solid.
Example: core.rs
              timeseries: self.timeseries.clone(),
That's a fucking massive bit of data. Cloning is very cheap trick. This is totally immutable data.
Anaylse all of our .clone() operations through all files. We can add lifetimes....

# Did we ever fix the hardcoded slider
needs to be min/max not 2..50 or whatever?

# Price change triggers
Retest with value very low value again here;
        price_recalc_threshold_pct: 0.01,
Just to be sure
Soon I will need retest this as well......
We need to test this as well....
        price_recalc_threshold_pct: 0.01,

# WASM mode
make sure WASM mode works - might fail now with new price stream init?
