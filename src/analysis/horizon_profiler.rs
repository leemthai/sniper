use crate::config::ANALYSIS;
use crate::config::PriceHorizonConfig;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::horizon_profile::{HorizonBucket, HorizonProfile};
use crate::models::timeseries::find_matching_ohlcv;

pub fn generate_profile(
    pair: &str,
    timeseries_collection: &TimeSeriesCollection,
    current_price: f64,
    config_ref: &PriceHorizonConfig, 
) -> HorizonProfile {
    let mut profile = HorizonProfile::new();
    
    let Some(ohlcv) = find_matching_ohlcv(
        &timeseries_collection.series_data,
        pair,
        ANALYSIS.interval_width_ms
    ).ok() else {
        return profile;
    };

    let min_pct = config_ref.min_threshold_pct; 
    let max_pct = config_ref.max_threshold_pct; 
    let steps = 100;
    
    // 1. Initialize Buckets
    let step_size = (max_pct - min_pct) / steps as f64;
    for i in 0..=steps {
        let pct = min_pct + (i as f64 * step_size);
        profile.buckets.push(HorizonBucket {
            threshold_pct: pct,
            candle_count: 0,
            duration_days: 0.0,
            // Initialize timestamps limits
            min_ts: i64::MAX,
            max_ts: 0,
        });
    }

    // 2. ONE PASS SCAN 
    // Track frequency AND min/max timestamps per bucket increment
    let mut frequency_map = vec![0usize; steps + 1];
    let mut min_ts_map = vec![i64::MAX; steps + 1];
    let mut max_ts_map = vec![0i64; steps + 1];
    
    let total_candles = ohlcv.klines();
    let start_ts = ohlcv.first_kline_timestamp_ms;
    let interval = ohlcv.pair_interval.interval_ms;

    for i in 0..total_candles {
        let candle = ohlcv.get_candle(i);
        
        // Calculate deviations
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
                let candle_ts = start_ts + (i as i64 * interval);
                
                frequency_map[start_index] += 1;
                
                // Track min/max for this specific slice
                if candle_ts < min_ts_map[start_index] {
                    min_ts_map[start_index] = candle_ts;
                }
                if candle_ts > max_ts_map[start_index] {
                    max_ts_map[start_index] = candle_ts;
                }
            }
        }
    }

    // 3. Cumulative Sum & Propagation
    let mut running_count = 0;
    let mut running_min_ts = i64::MAX;
    let mut running_max_ts = 0;

    for i in 0..=steps {
        // Accumulate Count
        running_count += frequency_map[i];
        
        // Accumulate Timestamps
        // If the current slice has data (min_ts != MAX), fold it into the running stats
        if min_ts_map[i] != i64::MAX {
            if min_ts_map[i] < running_min_ts { running_min_ts = min_ts_map[i]; }
            if max_ts_map[i] > running_max_ts { running_max_ts = max_ts_map[i]; }
        }

        // Update Profile
        let bucket = &mut profile.buckets[i];
        bucket.candle_count = running_count;
        
        let duration_ms = running_count as f64 * interval as f64;
        bucket.duration_days = duration_ms / (1000.0 * 60.0 * 60.0 * 24.0);
        
        // Store calculated bounds
        // If count is 0, we leave defaults (MAX/0) or set to 0? 
        // Let's leave defaults, logic in UI handles empty buckets.
        if running_count > 0 {
            bucket.min_ts = running_min_ts;
            bucket.max_ts = running_max_ts;
        }

        if running_count > profile.max_candle_count {
            profile.max_candle_count = running_count;
        }
    }

    profile
}