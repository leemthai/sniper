# Interval Switching Guide

This guide explains how to switch between different candle intervals (1h, 15m, 5m, etc.) in the klines system.

## Current Configuration

The system is currently set to **1-hour (1h) intervals**.

## How to Switch Intervals

All interval configuration is centralized in a single constant. To change intervals:

### Step 1: Update the Interval Constant

Edit `src/config/analysis.rs`:

```rust
pub const INTERVAL_WIDTH_TO_ANALYSE_MS: i64 = MS_IN_H;  // Current: 1 hour
```

**Available options:**
```rust
MS_IN_5_MIN   // 5-minute candles (300,000 ms)
MS_IN_15_MIN  // 15-minute candles (900,000 ms)
MS_IN_30_MIN  // 30-minute candles (1,800,000 ms)
MS_IN_H       // 1-hour candles (3,600,000 ms) [DEFAULT]
MS_IN_4_H     // 4-hour candles (14,400,000 ms)
MS_IN_D       // 1-day candles (86,400,000 ms)
```

All constants are defined in `src/utils/time_utils.rs`.

### Step 2: Delete Old Cache (if switching intervals)

The system caches kline data with interval-specific filenames. Old caches are automatically ignored, but you can manually clean up:

```bash
rm -f kline_data/kline_*.bin  # Delete all cached intervals
# Or keep specific intervals:
rm -f kline_data/kline_1h_v*.bin  # Only delete 1h cache
```

### Step 3: Rebuild and Run

```bash
cargo build
cargo run -- --prefer-api  # Force fetch from Binance API
```

## What Changes Automatically

The following adapt automatically when you change `INTERVAL_WIDTH_TO_ANALYSE_MS`:

✅ **Data fetching** (`src/data/timeseries/bnapi_version.rs`) - pulls the correct interval from Binance  
✅ **Analysis calculations** (`src/analysis/pair_analysis.rs`) - matches data by interval  
✅ **Auto-duration** (`src/domain/auto_duration.rs`) - adjusts minimum lookback candle counts  
✅ **Timestamp calculations** - all ms-based math uses `pair_interval.interval_ms`  
✅ **GUI displays** - shows actual interval via `pair_interval`  

## Impact of Different Intervals

### 15-Minute Candles (MS_IN_15_MIN)

**Pros:**
- 4x more granular than 1h (better captures intraday volatility)
- Better for short-term price discovery analysis
- More precise zone identification during rapid moves

**Cons:**
- 4x more data to fetch/store (longer load times)
- Requires 4x more API calls
- Cache files are larger

**Minimum lookback:**
- 7-day minimum = 672 candles (vs 168 with 1h)
- MIN_CANDLES_FOR_ANALYSIS = 100 (always sufficient with 15m)

### 5-Minute Candles (MS_IN_5_MIN)

**Pros:**
- 12x more granular than 1h
- Near real-time price action analysis

**Cons:**
- 12x more data overhead
- Much larger cache files
- Slower computation
- May be too noisy for longer-term patterns

**Minimum lookback:**
- 7-day minimum = 2,016 candles
- Excellent for high-frequency analysis

### 1-Hour Candles (MS_IN_H) [DEFAULT]

**Best balance** for most use cases:
- Good granularity without excessive data
- Reasonable API call count
- Fast computation
- 7-day minimum = 168 candles (still above MIN_CANDLES_FOR_ANALYSIS)

## Testing Different Intervals

### Recommended Test Process

1. **Start with 1h** (default) - establish baseline results
2. **Try 15m** - test if higher granularity improves edge detection
3. **Compare out-of-sample performance** - which interval produces better predictive zones?

### Quick Test Commands

```bash
# Test with 1h (default)
cargo run

# Switch to 15m
# 1. Edit `src/config/analysis.rs`: INTERVAL_WIDTH_TO_ANALYSE_MS = MS_IN_15_MIN
# 2. Delete cache
rm -f kline_data/kline_*.json
# 3. Run
cargo run -- --prefer-api

# Switch back to 1h
# 1. Edit `src/config/analysis.rs`: INTERVAL_WIDTH_TO_ANALYSE_MS = MS_IN_H
# 2. Delete cache
rm -f kline_data/kline_*.json
# 3. Run
cargo run -- --prefer-api
```

## Minimum Candle Count Protection

The system now validates that you have at least **100 candles** (`MIN_CANDLES_FOR_ANALYSIS`) in the selected time range.

**What triggers the error:**
- Rapid price discovery (15% pump in a few hours)
- Very short lookback period
- Auto-duration selecting too narrow a window

**Error message:**
```
Insufficient data for analysis: BTCUSDT has only 47 candles in the selected range.
Minimum required: 100. This typically occurs during rapid price discovery when
historical data at current price levels is limited.
```

**How to fix:**
- Use longer lookback periods (not always possible during price discovery)
- Use shorter intervals (15m gives 4x more candles than 1h for same time period)
- Wait for more historical data to accumulate

## Architecture Notes

### Why This Design Works

The system is **interval-agnostic** by design:

1. `PairInterval` struct stores `interval_ms` dynamically
2. All calculations use `timeseries.pair_interval.interval_ms` (not hardcoded values)
3. Single source of truth: `INTERVAL_WIDTH_TO_ANALYSE_MS`

### Why Empty Zones Matter

**Note:** A zone with zero volume is not "missing data"—it represents a **slippy zone** where price moved through quickly without resistance. This is valuable information for the trading model.

## Cache System Details

### Binary Format (Bincode)

The system now uses **bincode** instead of JSON:

**Performance improvements:**
- 🚀 **10-20x faster** serialization/deserialization
- 💾 **~3-5x smaller** file sizes (200MB JSON → ~50-70MB bincode)
- ⏱️ **Sub-second writes** instead of 15+ minutes

**Cache filename format:**
```
kline_data/kline_{interval}_{version}.bin

Examples:
kline_1h_v4.0.bin   # 1-hour interval cache
kline_15m_v4.0.bin  # 15-minute interval cache
kline_5m_v4.0.bin   # 5-minute interval cache
```

### Automatic Cache Management

- ✅ **Interval-specific**: Different intervals use separate cache files (no conflicts)
- ✅ **Version-aware**: Old cache versions are automatically ignored
- ✅ **Non-blocking writes**: Cache writing happens in background thread (UI doesn't freeze)
- ✅ **Validation**: Cache age, version, and interval are checked on load

### Cache Write Behavior

Cache is written **asynchronously after data fetch**:
1. App loads data (from cache or API)
2. GUI starts immediately
3. Cache write happens in background if data came from API
4. You can close the app before cache write completes (though not recommended)

## Configuration Summary

```rust
// src/config/analysis.rs
pub const DEFAULT_PRICE_ZONE_COUNT: usize = 200;      // Fixed at 200 zones
pub const MIN_CANDLES_FOR_ANALYSIS: usize = 100;      // Minimum data requirement
pub const INTERVAL_WIDTH_TO_ANALYSE_MS: i64 = MS_IN_H; // ← CHANGE THIS to switch intervals
```

**Only change `INTERVAL_WIDTH_TO_ANALYSE_MS`** - everything else adapts automatically, including cache filenames.
