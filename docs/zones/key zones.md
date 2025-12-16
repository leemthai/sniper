
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


# Note: Don't forget any time we print prices, use format_price() instead of just ${:.2} or whatever.
Fixed via format_price(), always

# 0.35
See when 0.35 version is due out and what features it will offer. Might help guide decision making
Appears like quite a big API change (sigh)
https://github.com/emilk/egui_plot/issues/200
# https://github.com/emilk/egui_plot/pull/221
egui_plot API update. it's for 0.35 version right? We can't use it yet?


# Are we serializing too much? (vague guess)
Have a look at state file soon in my spare time
Get AI to have a look at state.json file. He can analyse

# AI Question X (random stuff. not posted yet)
ok that feels much better. I will test SIM more tmr (how to test if model is working when we sim price.... dunno. the model doesn't care much about live price does it)
Just out of curiosity, where does the trading model actually use the current price? It's not used much is it, coz the model is mostly about zones rn rather than relationships with price.
What about SIM mode for a pair where model is down. SHould we still SIM mode to run? What is point with no model??? Dunno.
Does it save sim mode? Nope. That's fine.
And then test this as well:
        price_recalc_threshold_pct: 0.01,
Then we can audit cloning and see if anything to fix (though I don't want get into another massive debugging day. Just wanna check if we have massive clones going on we can avoid. Small clones are fine tbh.)

# Price change triggers
Retest with value very low value again here;
        price_recalc_threshold_pct: 0.01,
Just to be sure
Soon I will need retest this as well......
We need to test this as well....
        price_recalc_threshold_pct: 0.01,

# Why have low wick and high wick zones largely disappeared?
No idea. Maybe just the new pair collection? Nah
OR they just tend to disappear as you reduce PH?

# Info: New repo is called 'sniper'
Easy enough to rename if needed
Pages is set up but app not running coz coding issues (see WASM Version not running)

  
# Tuesday
Completey get rid of crate:: calls from app.rs coz I pasted that all in again.... and main.rs and anywhere else....
Do this once app is running.... don't do yet coz I will probably paste his code in again, and he wrill overwrite my overwrites.
app_simulation.rs too
WASM version - what is it loading from? json or ... must be coz we don't have DB in WASM, lol.

# Note - to remove db files
cd rust/wherever lol
rm klines.sqlite*


# For later when we have basic DB system working better

- PH Report when app is running
I haven't played with the app much 
Number of candles jumps around in PH when you drag the bar. In a weird way. not a good way. So for LUNATRY it can go from 32672 candles to 306 candles but then you release the mouse, and it goes back to 32672. Very weird
Yeah, all screwed up again. move into red zone and still shows map. Need debug

- Gaps in data
Are we handling gaps at all yet?
If so, how?
Because some pairs have *severe* gaps of years.

- In-app calculations
Are we calculating everything correctly still?
For BTCUSDT it says:
Evidence: 8Y 4M (52673 Candles)
History: 8Y 4M
Is that correct number of candles for 8Y for 5M candles?

- Updating DB in background when app is running
Do we do that yet? If so on what schedule?

- get rid of allow(unused_imports) as directive
It needs to specify exactly under which build circumstance we need the allow...
pre_main_async.rs
#[allow(unused_imports)] // Bit shitty this
use anyhow::Result;
So i guess that will be non WASM version we need it?

# make_demo_cache
will need rewriting, right? Coz no idea what it does rn but bound to be wrong.....
investigate. then can dump a load of old code like create_timeseries_data, serde_version etc.
We don't have serde_version anymore

# create_timeseries_data
called by serde_version.
but wtf is serde_version anymore.