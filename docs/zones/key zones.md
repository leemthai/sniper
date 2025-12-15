
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

# Can we now delete min_lookback_days from anaylsis?
Piece of shit.


# Info: New repo is called 'sniper'
Easy enough to rename if needed
Pages is set up but app not running coz coding issues (see WASM Version not running)




# The future of klines
Loading candles is currently a vary long operation. Not something you can easily play aroind with on the fly
Takes 2 mins to load 30m klines for 70 pairs. Coz we keep reloading from scratch all the time. need to learn  how to buffer them locally. Then we could do 5 mins
5 mins 100 candles is 500 mins is 8 hrs. Need evaluate how much we slow various app operations if we go down to 5 min candles from 30 min candles..
Store in local db. what are rust db options
Re-loading all klines from API every day is dumb and very very slow. They should be added incrementally to a db in the background after one 'big load' when app first run (which writes db to local disk)
Also eventually want to load in background then trigger zone recalcs etc. 
5m klines else maths gets too slow maybe. Processing all the klines i mean.
Don't forget this is for non-WASM only. WASM mode does not load klines from API at all. Though if we switch to DB, I guess we probably switch WASM-verison to local db as well?

# Work with any kline interval
For the ulitmate flexbility, have a global option to use any reasonable interval.
Then, for slower machines, less local storage, or when a person is doing swing-trading / investing, they can pick a bigger interval
For a person doing scalping, they could pick one minute
This is 'big' global option though, would mean rebuilding the db etc. So a complete app relaunch.
Definite good to keep it flexible like this though.
If we pin it down to one particular interval, we are not writing an app that can adapt

# Can we use 1m or 5m klines all the time
And somehow aggregate them if we want to reduce the calc time etc
Get the best of all worlds
Then the user does not need to select a kline interval, or if he does, it does not trigger any kind of data reload from API, we just deal with it internally...

# If we redo klines
Note that Binance klines have holes in them. Some very big holes. Currently I attempt to 'fill in holes' in some way.
Might it be an opportunity to turn instead to a bunch of kline ranges? Similar to how we store 'qualifying klines' in the app itself when calculating zones etc?
Need to decide how to start-up the app ...... do we launch egui to start? or do a whole pre-egui section where we run some other kind of interface to load klines, and do whatever else might need doing?
Investigate other binance crates. This one is fine, but maybe another is finer? I remember you saying that binance-sdk had an awkwared API and was auto-generated, or something like that. Maybe we can find a better one.

# DB options (if we go redo klines)
Polo db
Native db
These are 'no SQL' options