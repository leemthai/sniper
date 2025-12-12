
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

# WASM version not running rn.
http://leemthai.github.io/sniper/
zone-sniper-81a5c0fd4d43cb15.js:751 panicked at /rustc/ded5c06cf21d2b93bffd5d884aa6e96934ee4234/library/std/src/thread/mod.rs:731:29:
failed to spawn thread: Error { kind: Unsupported, message: "operation not supported on this platform" }
Stack:

Error
    at imports.wbg.__wbg_new_358bba68c164c0c7 (zone-sniper-eb14e317b50740a6.js:1226:21)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.eframe::web::panic_handler::Error::new::__wbg_new_358bba68c164c0c7::h85c5d2c0dc73e10b externref shim (zone-sniper-eb14e317b50740a6_bg.wasm:0x65ef22)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.eframe::web::panic_handler::Error::new::hf000dd9e39e74d0c (zone-sniper-eb14e317b50740a6_bg.wasm:0x5e8558)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.eframe::web::panic_handler::PanicSummary::new::h6e2fb215abe07905 (zone-sniper-eb14e317b50740a6_bg.wasm:0x52137e)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.eframe::web::panic_handler::PanicHandler::install::{{closure}}::h6846f31d7cf5f53c (zone-sniper-eb14e317b50740a6_bg.wasm:0x36baae)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.std::panicking::panic_with_hook::h2f3f743d7642d6f0 (zone-sniper-eb14e317b50740a6_bg.wasm:0x4955a3)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.std::panicking::panic_handler::{{closure}}::h4d243ab0bfd167e5 (zone-sniper-eb14e317b50740a6_bg.wasm:0x50a57e)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.std::sys::backtrace::__rust_end_short_backtrace::hf9fb4031b9f27768 (zone-sniper-eb14e317b50740a6_bg.wasm:0x660606)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.__rustc[eb8946e36839644a]::rust_begin_unwind (zone-sniper-eb14e317b50740a6_bg.wasm:0x62bdf5)
    at zone_sniper-a8cd1f0fd9f35d5d.wasm.core::panicking::panic_fmt::h0ce8f0f8ae811b17 (zone-sniper-eb14e317b50740a6_bg.wasm:0x62be7e)


My Thought: When you changed the startup code maybe  you run some code  in wasm mode that cant be run on wasm. Some kind of thread code it seems.... but its hard to seee from the stack trace above where the code is?

# Plus this is shit in wasm_demo.rs
// TEMP very dodgy code here. Hard-coding the filename to load. Will fail as soon as we switch to a different interval
const DEMO_CACHE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/kline_data/demo_kd_30m_v4.bin"
));
Needs to use config/binanace.rs or something
for both 'kline_data' and 'demo_kd' etc
Always use config/binance
Search my code for kline_data etc


# Info: New repo is called 'sniper'
Easy enough to rename if needed
Pages is set up but app not running coz coding issues (see WASM Version not running)


# Trading pairs which are stablecoin -> stablecoin
Inherently % movements are very small.......
So even 1% is a lot.
How to deal with these?
They need a lower limit somehow.
Perhaps implies price range should be per-pair not global??????

# Per pair price horizon settings
As part of stablecoin thing.
Why? Because a 0.1% change in stablecoin price is needed because 'volatility'

# Up the intensity of the background bars
But add key to allow it to be turned off as well.
Still hate using "B" to rotate meaning of background bar. It's so awkward.
