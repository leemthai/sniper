
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



# New thingf to tell him when I have time
# Price change triggers
TRIGGER: Seems to be working (can put that back to 0.01 in analyisconfig)
But ...... it keeps printing many many times.
Some kind of queue flooding maybe

I thought we ran the model before we render the UI. So why does it say 'calculating zones' when you click to a different pair.
I think it could be because of the queue, and in debug mode, a queue item takes much longer to run?
It should never ever say that now, right?

# In case of `Not enough zones` - what happens now?
We have a constant somewhere which describes the minimum number of qualifying candles in the price range
min_candles_for_analysis
What happens if our pair does not have enough candles to qualify for analysis. We should have specific message come up right? Do we have such a specific message now?


# Too much cloning is making me itch:
Example: core.rs
              timeseries: self.timeseries.clone(),
That's a fucking massive bit of data. Cloning is very cheap trick. This is totally immutable data.
Anaylse all of our .clone() operations through all files. We can add lifetimes....

# The Queue
Just seems to go up - never down. And up to 812..... There should only EVER be one queue job per PAIR.
I thought I gave you some good queue logic in design dude?!?
If price horizon is changed (i.e. global invalidation), we replace current queue with new queue. We don't add to the fucking end. What the fuck?????
Don't fuck up this new code dude. I want it tight.
Show me thee current queue logic, and how you plan to make it more intelligent....

Also, the queue number only seems to go down when I move the mouse over the app. What the fuck is that? Please investigate? That is actually a thing. The queue number stays fixed unless you move the mouse over the app. Honestly. If I leave the app alone for a minute and then move the mouse over it again, the queue number will start counting down again. What happened to the model running fully indepedently?

No new code changes rn. Just talk to me about the issues raised
