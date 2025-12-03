# Definitions

**pair**
- **aliases**: **symbol**, **trading pair**, **asset pair**
- **defn**: a pair of assets that can be traded as an entity on a trading exchange.
  
**price**
- **aliases**: **price point**, **price level**, **current price**, **live price**, **pair price**
- **defn**: the current price of a `pair`.
    
**time interval of interest**
- **defn**: a time period (or series of time periods) to analyse. Does not have to be contiguous. For example, if we analyse price action +/- % of current price, the price action will not typically be contiguous. 
  
**price horizon** 
- **aliases**: **price range of interest**
- **defn**: a range of prices to analyse. For example, +/- 10% of `pair price`. Does not have to be contiguous but in current system, it always is.
- **how set**: in UI by the user.

**time horizon**
- **aliases**: **max journey time**
- **defn**: The maximum duration allowed for a journey (i.e trade opportunity) to reach its target before it is treated as a timeout. In other words, the outer temporal limit that the user is interested in i.e. "make me money (or lose it) before this time is up".
- **how set**: in UI by the user.

**zone**
**aliases**: **area**, **price zone**, **level**
- **defn**: a fixed-size range of price action for a `pair`.
- **see also**: **superzone**
  
**superzone**
- **defn**: aggregation of `zone` objects i.e. turns fixed-width `zone` into variable-sized item.
- **see also**: `zone`

**target zone** (previously called `key zone`)
- **defn**: a price (range) which the system + user together has decided to target.
 
**sticky zone** (basis of zones/superzones in app)
- **aliases**: **high volume node**, **HVN**, **high volume zone**, **high friction zone**, **gravity well zone**, **support/resistance zone**, **SR zone**, **popular trading zone**
- **defn**: zones with historically high trading volume represent areas where a large number of buyers and sellers have previously agreed on a price, which can act as significant support or resistance levels. These areas indicate strong market interest and can be zones where prices may stall, reverse, or find fair value. Traders use them to identify potential entry or exit points and to confirm trend strength
- **What these zones represent**:
  - **Support and resistance**: a high-volume zone from a previous downtrend may act as a support level on a future pullback. Conversely, a high-volume zone from a previous uptrend can serve as resistance.
  - **Fair value and market consensus**: these levels are where the market previously found consensus between buyers and sellers, suggesting it's a price level with a significant agreement on value.
  - **Institutional interest**: high-volume nodes can often highlight areas where large institutions have built or reduced positions, making them key levels to watch

**liquidity zone** (currently unused in app)
- **defn**: zone where there is a significant concentration of buy or sell orders, often indicated by high volume, consolidation, or strong price reactions. These zones act as magnets for price, representing potential areas where large market participants, like institutions, may enter or exit positions, making them important for identifying future price movements.
- **Characteristics**: 
  - **High volume**: indicates a significant concentration of buy or sell orders.
  - **Consolidation**: prices tend to consolidate within these zones, indicating a lack of clear direction.
  - **Historical support/resistance**: levels that have repeatedly acted as support or resistance in the past.

**Consolidation zone** (currently unused in app, but is this just **sticky zone** anyway)
- **defn**: zone where prices tend to consolidate within a fixed-size range of price action over a certain interval of interest. These zones often indicate a lack of clear direction and can be used to identify potential areas of support or resistance.

**slippy zone** (currently unused in app I believe)
- **aliases**: **low friction zone**, **transit zone**, **low volume zone**, **low gravity zone**
- **defn**: the opposite of `sticky zone` i.e. zone where least amount of trading takes place.
  
**reversal zones** (currently unused in app)
- **aliases**: `rejection zones`,`unstable zones`
- **defn**: price tends to reject here and REVERSE direction.
    
**low wick zone** (currently unused in app)
- **defn**: a `reversal zone` made of up just low wicks.
    
**high wick zone** (currently unused in app)
- **defn**: a `reversal zone` made of up just high wicks.

**start price**
- **defn**: start price of a journey or trading opportunity. This is (typically) not a zone. It's just the `live price`. Journeys begin at the price now, rather than key zones, allowing the user to spot opportunities today, not unspecified date in future.
- **see also**: `docs/journeys/journey_spec.md`

**end price**
- **defn**: end price of a journey or trading opportunity. In current implementation, is always the nearest edge of an `key zone` within the `price horizon` defined by the user.
- **see also**: `docs/journeys/journey_spec.md`
    
**historical price ourney analysis**
- **aliases**: **HPJA**
- **see also**: `docs/journeys/journey_spec.md`
- **defn**: a method of analysing historical price action of a pair to come up with high value trading opportunities. Full explanation in `docs/journeys/journey_spec.md`
