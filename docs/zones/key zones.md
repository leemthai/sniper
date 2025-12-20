
## (Journey stuff) Sticky zone price target
Shoud price target be center rather than nearest edge of sticky zone ? (seems more natural than aiming for edge of structure, right?)
That's journey stuff I think, though, don't want let Gemimi loose on that yet, though, lol

# Target zone should be represented with a circle (the sniper target zone)
Cooolllll!!!!!!!!!!!!!!!!!!

# Stop printing hover windows in random colors
I want to print in fixed colors somehow

# Differentiate stablecoins in pair list
Be nice to mark stablecoin pairs maybe, separate them in the list or mark them at least. Then they could have a different default value for PH. Currently it is 15% for everything which is not very friendly. If we separate them out, would mke more sense to give them different default.

# Test full DB Rebuild
Time how long it takes in release mode
Can archive current db. That will force a full rebuild I guess
Note: to remove db files if we want to test from fresh
cd rust/wherever lol
rm klines.sqlite*

# Updating DB in background when app is running
Do we do that yet? If so on what schedule?
Not even thoought about that yet.
Ask AI how big a job. Where to put in schedule?b

# How to do deal with pairs 'on the up' but have no significant sticky zones above them
ie.. these pairs will not find a higher target zone so can never target it. Sad
PAXGUSDT
but maybe pairs which have recently been in price discovery will be like this
Zoom in? Yep. that works.

# Codium keys
Ctrl+P to search through files quikly
Ctrl+T: Global Symbol Search to search for functions+structs+varaibles maybe
Ctrl+Shift+E - file explorer
Ctrl+Shift+I - Rust formatting
Ctrl+Shift+F - Global search


# Note: Don't forget any time we print prices, use format_price() instead of just ${:.2} or whatever.
Fixed via format_price(), always

# egui_plot 0.35 coming soon 
See when 0.35 version is due out and what features it will offer. Might help guide decision making
Appears like quite a big API change (sigh)
https://github.com/emilk/egui_plot/issues/200
# https://github.com/emilk/egui_plot/pull/221
egui_plot API update. it's for 0.35 version right? We can't use it yet?

# AI Question X (random stuff. not posted yet)
ok that feels much better. I will test SIM more tmr (how to test if model is working when we sim price.... dunno. the model doesn't care much about live price does it)
Just out of curiosity, where does the trading model actually use the current price? It's not used much is it, coz the model is mostly about zones rn rather than relationships with price.
What about SIM mode for a pair where model is down. SHould we still SIM mode to run? What is point with no model??? Dunno.
Does it save sim mode? Nope. That's fine.
And then test this as well:
        price_recalc_threshold_pct: 0.01,
Then we can audit cloning and see if anything to fix (though I don't want get into another massive debugging day. Just wanna check if we have massive clones going on we can avoid. Small clones are fine tbh.)

# Info: New repo is called 'sniper'
Easy enough to rename if needed
Pages is set up but app not running coz coding issues (see WASM Version not running)
  


# ideas to ask AI
ask him to improve the project; either coding-based (features, coding style,  DRY, project organization, rayon for easy paralllelization, whatever u can think of) or front-end software (ui), or actual trading features (which are very underfeatured so far. We have hardly get started yet on this...)
Why do I ask? Because when we make an improvement, you always say "this improvement has upgraded your code from amateur to professional level" but you never tell me in advance "you should really upgrade this part of the system.... so I was hoping maybe you could suggest a thing or two I have never thought of...."
other binance APi services?
other public crypto services that might be useful?

# Random AI Advice
"Check `src/main.rs` for idiomatic Rust patterns and refactor imports."


