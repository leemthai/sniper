The old "Phase 2" stuff about journeys:

## My 2 cents to add to this document.....
 HOWEVER, PLEASE BEAR IN MIND THAT THE HISTORICAL JOURNEYS WE HAVE ENCOUNTERED and logged, ALREADY TAKE THESE ZONES INTO ACCOUNT (because more journeys get completed in slippy zones etc. So maybe we cannot add this idea into the system. It's "Counting this stuff double"? 
 
  The most important thing we can do in our system is analyse the zones between the start and end price. If it is full of low scores, means there is little volume between here and target zone. Means high probabilitity of success. A simple algorithm would:
    1. Journey average score = Take average score from here to target price.
    2. If journey average score is less than overall average score across all zones, then increase the probability of success. If journey average score is more than overall average score, decrease the probability of success.
  Plus other factors:
  1. Reversal zones in between here and target price.
  2. etc.

## I then asked this question of Claude:
"If for a given journey we have factors that we know would either increase or decrease the chance of a journey succeeding (so not filters, but factors that affect the probability of success), how could we hope to incorporate them into this model, and if we can, at what phase(s)?"

and got very very long answer..... which is summarized below....

## Incorporating Success-Influencing Factors into the Probability Model
### Conceptual Approach
Instead of filtering out historical data, you're asking how to weight or adjust probabilities based on factors that make success more or less likely. This is about building a conditional probability model.
### Types of Factors to Consider
#### Directional Factors (Increase/Decrease Probability)
1. Market regime: Bull vs bear market i.e current market trends. Are we in up-trend, down-trend? And this has to relate to the what time period?
  I guess, the max time period for the journey? Something like that. I mean, if the user wants this trade to complete within a week, do we look at the last week trend or something? Or a few trends at different time periods? Unknown......
2. Volatility state: High vs low ATR at journey start
3. Volume profile: Above/below average volume at start price
4. Momentum: Recent price velocity (rising into start_price vs falling)
5. Support/Resistance proximity: Is end_price near historical resistance?
6. Broader market correlation: BTC trend if analyzing altcoins i.e. altcoins tend to go up when BTC goes up, and "more so". However, this is overcomplicating I feel..... altcoins can break out on their own even with BTC is flatlining. And market correlations change all the time. Worth thinking about, though, obviously. If BTC is in up-trend, that naturally makes chances of altcoins going up increase, and vice versa (but need establish correlations first, don't "presume correlations")
#### Example
Historical data shows 60% success rate overall, but:
1. When starting during high volume: 75% success
2. When starting during low volume: 45% success
3. When BTC is bullish: 70% success
4. When BTC is bearish: 50% success

## Integration Methods (Ranked by Complexity)

### Method 1: Stratified Probability (Simplest)
  What: Group historical observations by factor values, calculate separate probabilities.
  When to Apply: Phase 4 (Probability Estimation)
How It Works:
1. Divide historical observations into buckets based on factor values
2. Calculate P(success) separately for each bucket
3. At prediction time, use the probability from the bucket matching current conditions
### Example:
1. High volume historical attempts: 20 successes / 25 attempts = 80%
2. Low volume historical attempts: 15 successes / 40 attempts = 37.5%
3. Current prediction: If volume is currently high → use 80%
### Limitation: Requires enough data in each bucket (minimum 10-20 per bucket)

### Method 2 : Weighted Historical Matching (Moderate)
  What: Assign relevance weights to each historical observation based on similarity to current conditions.
  When to Apply: Phase 1 (Data Extraction) + Phase 4 (Probability)

### Method 3 : Logistic Regression Model (Advanced)
  What: Train a statistical model where factors are independent variables predicting success/failure.
  When to Apply: Phase 4 (Probability Estimation) - replaces simple counting

### Method 4: Bayesian Update (Most Sophisticated)
  What: Start with base probability, update it based on observed factors using Bayes' theorem.
  When to Apply: Phase 4 (Probability) + Phase 5 (Expected Value)

## Critical Implementation Notes
### Data Requirements by Method

#### Method 1 (Stratified):
  Minimum: 30 total observations
  Each bucket needs 10+ samples
  Works with 2-3 factors maximum

#### Method 2 (Weighted):
  Minimum: 50 total observations
  Can handle 4-6 factors
  More robust to sparse data

#### Method 3 (Regression):
  Minimum: 100 total observations
  Rule of thumb: 10-20 observations per factor
  5-8 factors maximum with 100-200 samples

#### Method 4 (Bayesian):
  Works with any sample size
  Requires domain knowledge for priors
  Best for combining data sources

## Factor Selection Discipline
### Avoid Overfitting:
  Don't use more factors than you have data to support
  Test factor predictive power before including
  Factors must be known at journey start, not hindsight
### Bad Factors (Don't Use):
  "Maximum price reached during journey" → That's the outcome!
  "Whether news event happened during journey" → Unknown at start
  "Final trend direction" → Hindsight bias
### Good Factors (Use):
  Volume at journey start
  Volatility in 14 days before start
  Broader market trend before start
  Day of week, time of year
  Price position relative to moving averages

## Recommended Approach for Your Use Case (IGNORE THIS FOR NOW COZ I PREFER BAYESIAN OPTION TBH)
### Phase-by-Phase Integration:
### Phase 1 Enhancement
Collect these specific factors for each historical journey:
  Volume (absolute)
  Volume ratio (current / 30-day average)
  ATR-14
  Price vs 50-day MA (above/below percentage)
  7-day momentum (percentage change)
### Phase 4 Core Change
Implement Method 2 (Weighted Matching):
  Calculate similarity scores using Euclidean distance
  Weight each historical journey by similarity
  Use weighted averages for all statistics
#### Phase 5 Extension
  Compute scenario analysis (best/base/worst case EV)
  Show sensitivity: "If volume were 20
