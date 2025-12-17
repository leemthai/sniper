use crate::config::ANALYSIS;
use crate::config::PriceHorizonConfig;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::horizon_profile::{HorizonBucket, HorizonProfile};
use crate::models::timeseries::find_matching_ohlcv;

// ... imports ...

pub fn generate_profile(
    pair: &str,
    timeseries_collection: &TimeSeriesCollection,
    current_price: f64,
    config_ref: &PriceHorizonConfig, 
) -> HorizonProfile {
    let mut profile = HorizonProfile::new();
    
    // Store validation data
    profile.base_price = current_price;
    profile.min_pct = config_ref.min_threshold_pct;
    profile.max_pct = config_ref.max_threshold_pct;

    // 1. Find Data
    let Some(ohlcv) = find_matching_ohlcv(
        &timeseries_collection.series_data,
        pair,
        ANALYSIS.interval_width_ms
    ).ok() else {
        return profile;
    };

    // ... (Rest of logic remains EXACTLY the same as previous correct version) ...
    // Just ensure you keep the `timestamps` logic we fixed earlier.
    
    // Copy the implementation from my previous message, just adding the 3 lines above.
    
    let min_pct = config_ref.min_threshold_pct; 
    let max_pct = config_ref.max_threshold_pct; 
    let steps = config_ref.profiler_steps.max(100); // Safety floor
    
    let step_size = (max_pct - min_pct) / steps as f64;
    for i in 0..=steps {
        let pct = min_pct + (i as f64 * step_size);
        profile.buckets.push(HorizonBucket {
            threshold_pct: pct,
            candle_count: 0,
            duration_days: 0.0,
            min_ts: i64::MAX,
            max_ts: i64::MIN, 
        });
    }

    // 3. ONE PASS SCAN 
    let mut frequency_map = vec![0usize; steps + 1];
    let mut min_ts_map = vec![i64::MAX; steps + 1];
    let mut max_ts_map = vec![i64::MIN; steps + 1];
    
    let total_candles = ohlcv.klines();

    for i in 0..total_candles {
        let candle = ohlcv.get_candle(i);
        let candle_ts = candle.timestamp_ms;
        
        let dist_below = if candle.high_price < current_price {
            (current_price - candle.high_price) / current_price
        } else { 0.0 };

        let dist_above = if candle.low_price > current_price {
            (candle.low_price - current_price) / current_price
        } else { 0.0 };

        let required_pct = dist_below.max(dist_above);
        
        if required_pct <= max_pct {
            let raw_index = ((required_pct - min_pct) / step_size).ceil();
            let start_index = if raw_index < 0.0 { 0 } else { raw_index as usize };
            
            if start_index <= steps {
                frequency_map[start_index] += 1;
                if candle_ts < min_ts_map[start_index] {
                    min_ts_map[start_index] = candle_ts;
                }
                if candle_ts > max_ts_map[start_index] {
                    max_ts_map[start_index] = candle_ts;
                }
            }
        }
    }

    // 4. Cumulative Sum
    let mut running_count = 0;
    let mut running_min_ts = i64::MAX;
    let mut running_max_ts = i64::MIN;

    for i in 0..=steps {
        running_count += frequency_map[i];
        if min_ts_map[i] != i64::MAX {
            if min_ts_map[i] < running_min_ts { running_min_ts = min_ts_map[i]; }
            if max_ts_map[i] > running_max_ts { running_max_ts = max_ts_map[i]; }
        }

        let bucket = &mut profile.buckets[i];
        bucket.candle_count = running_count;
        
        if running_count > 0 {
            let duration_ms = running_max_ts.saturating_sub(running_min_ts);
            bucket.duration_days = duration_ms as f64 / (1000.0 * 60.0 * 60.0 * 24.0);
            bucket.min_ts = running_min_ts;
            bucket.max_ts = running_max_ts;
        } else {
            bucket.duration_days = 0.0;
        }

        if running_count > profile.max_candle_count {
            profile.max_candle_count = running_count;
        }
    }

    profile
}