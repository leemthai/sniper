
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

# Note - to remove db files if we want to test from fresh
cd rust/wherever lol
rm klines.sqlite*

- In-app calculations
Are we calculating everything correctly still?
For BTCUSDT it says:
Evidence: 8Y 4M (52673 Candles)
History: 8Y 4M
Is that correct number of candles for 8Y for 5M candles?

- Updating DB in background when app is running
Do we do that yet? If so on what schedule?

# DB Stuff
test from scratch building again, in both debug and release modes

# ideas to ask AI
ask him to improve the project; either coding-based (features, coding style,  DRY, project organization, rayon for easy paralllelization, whatever u can think of) or front-end software (ui), or actual trading features (which are very underfeatured so far. We have hardly get started yet on this...)
Why do I ask? Because when we make an improvement, you always say "this improvement has upgraded your code from amateur to professional level" but you never tell me in advance "you should really upgrade this part of the system.... so I was hoping maybe you could suggest a thing or two I have never thought of...."
other binance APi services?
other public crypto services that might be useful?

# Random AI Advice
"Check `src/main.rs` for idiomatic Rust patterns and refactor imports."
