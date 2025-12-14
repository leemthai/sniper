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
        });
    }

    // 2. ONE PASS SCAN (O(N) instead of O(N*100))
    // We categorize every candle into the "Minimum % Horizon" needed to see it.
    let mut frequency_map = vec![0usize; steps + 1];
    let total_candles = ohlcv.klines();

    for i in 0..total_candles {
        let candle = ohlcv.get_candle(i);
        
        // Calculate deviation from current price
        let dist_below = if candle.high_price < current_price {
            (current_price - candle.high_price) / current_price
        } else { 0.0 };

        let dist_above = if candle.low_price > current_price {
            (candle.low_price - current_price) / current_price
        } else { 0.0 };

        // The candle is visible if the Horizon % is larger than its distance
        let required_pct = dist_below.max(dist_above);
        
        // Find which bucket this candle *starts* appearing in
        if required_pct <= max_pct {
            let raw_index = ((required_pct - min_pct) / step_size).ceil();
            // Clamp to valid range (0 to steps)
            let start_index = if raw_index < 0.0 { 0 } else { raw_index as usize };
            
            if start_index <= steps {
                frequency_map[start_index] += 1;
            }
        }
    }

    // 3. Cumulative Sum
    // If a candle is visible at 2%, it is also visible at 3%, 4%, etc.
    let mut running_count = 0;
    for i in 0..=steps {
        running_count += frequency_map[i];
        
        profile.buckets[i].candle_count = running_count;
        
        let duration_ms = running_count as f64 * ohlcv.pair_interval.interval_ms as f64;
        profile.buckets[i].duration_days = duration_ms / (1000.0 * 60.0 * 60.0 * 24.0);
        
        if running_count > profile.max_candle_count {
            profile.max_candle_count = running_count;
        }
    }

    profile
}