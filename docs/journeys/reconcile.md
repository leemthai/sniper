# Very important !!! (can also be found as stage 0 in `docs/journeys/theoretical_model.md`)
There is mismatch between my original intent and what HPJA current does.
Plus, if we implement stage 0, we don't actually need Stage 1, because stage 1 is from zone to arbitrary price. What I want is live price to zone, lol. What a fuck-up
AI estimates 3-5 days to implement stage 0, so makes no sense to try it before 25th deadline. It's too tight.
GPT-5.1 (low reasoning) is pretty darned good, though. Quick plus codes well plus insightful (even though `low` reasoning, apparently.)

1. Original intent:
What you describe is:
  Start: always at current price, wherever it is.
  Goal: reach a sticky consolidation zone (as target), then maybe from there to some further TP.
  Sticky zone = target region, not “entry region”.
  Question is closer to:
    “From here (live price), how do paths behave relative to that consolidation zone?” i.e a live price -> zone mode.

2. What HPJA currently does (recap)
Current engine is:
  Entry condition: “price is already in the zone (± tolerance around current live price)”.
  Journey: from zone → target edge, with SL + Time Horizon.
  Sticky zone is being treated as entry context, not as a remote target.

So:
  Current HPJA answers:
    “When we were already sitting in this kind of zone, how did trades from here behave?”
  Your mental picture wants:
    “From wherever we are now, how do paths behave when we try to travel into that sticky zone?”
Those are related but genuinely different questions.

3. Reconcile the two:
Conceptually, what you want is a two-stage model:

1. Stage 0 – From live price to sticky zone (as target)
  Start at P_live.
  Target region = some consolidation band [Z_low, Z_high].
  Ask: “How often / how quickly do we enter this zone, and with what pain along the way?”

2. From zone to TP/SL/horizon (what HPJA already does)
  Condition on "now in zone J"
  Then apply the current HPJA rules....

Right now we only have Stage 1 implemented.

So:
  Your intent: live → zone (sticky as target).
  Current engine: zone → target (sticky as context).
They’re compatible, but not the same. The good news is:
  We don’t need to throw HPJA away.
  We can add Stage 0 later (live → zone model) and then compose it with existing Stage 1.
