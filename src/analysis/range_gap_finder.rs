use crate::models::OhlcvTimeSeries;
use crate::utils::time_utils;

#[derive(Debug, Clone, PartialEq)]
pub enum GapReason {
    None,              // Start of data
    PriceMismatch,     // Indices were skipped (Price out of PH)
    MissingSourceData, // Indices contiguous, but Time jumped (Exchange down/Delisted)

    PriceAbovePH,     // Excluded because Price > Max PH
    PriceBelowPH,     // Excluded because Price < Min PH
    PriceMixed,       // Generic/Mixed (rare, if price teleported across range)
}

#[derive(Debug, Clone)]
pub struct DisplaySegment {
    pub start_idx: usize,
    pub end_idx: usize,
    pub start_ts: i64,
    pub end_ts: i64,
    pub candle_count: usize,
    
    // Gap *preceding* this segment
    pub gap_reason: GapReason,
    pub gap_duration_str: String,
}

pub struct RangeGapFinder;

impl RangeGapFinder {
    pub fn analyze(
        timeseries: &OhlcvTimeSeries, 
        ph_ranges: &[(usize, usize)],
        price_bounds: (f64, f64)
    ) -> Vec<DisplaySegment> {
               if ph_ranges.is_empty() || timeseries.timestamps.is_empty() {
            return Vec::new();
        }

        let mut segments = Vec::new();
        let interval_ms = timeseries.pair_interval.interval_ms;
        let tolerance_ms = (interval_ms as f64 * 1.1) as i64; 

        let mut prev_segment_end_idx = 0;
        let mut prev_segment_end_ts = 0;
        let mut first_segment = true;

        for &(range_start, range_end) in ph_ranges {
            if range_start >= timeseries.timestamps.len() { continue; }
            let safe_end = range_end.min(timeseries.timestamps.len());
            
            let mut current_sub_start = range_start;
            
            for i in range_start..safe_end {
                let current_ts = timeseries.timestamps[i];
                
                if i + 1 < safe_end {
                    let next_ts = timeseries.timestamps[i+1];
                    let diff = next_ts - current_ts;
                    
                    if diff > tolerance_ms {
                        let sub_end = i + 1;
                        
                        segments.push(Self::create_segment(
                            timeseries, 
                            current_sub_start, 
                            sub_end, 
                            prev_segment_end_idx, 
                            prev_segment_end_ts,
                            first_segment,
                            price_bounds
                        ));
                        
                        first_segment = false;
                        prev_segment_end_idx = sub_end;
                        prev_segment_end_ts = timeseries.timestamps[sub_end - 1];
                        
                        current_sub_start = i + 1;
                    }
                }
            }
            
            if current_sub_start < safe_end {
                segments.push(Self::create_segment(
                    timeseries,
                    current_sub_start,
                    safe_end,
                    prev_segment_end_idx,
                    prev_segment_end_ts,
                    first_segment,
                    price_bounds
                ));
                first_segment = false;
                prev_segment_end_idx = safe_end;
                if safe_end > 0 {
                    prev_segment_end_ts = timeseries.timestamps[safe_end - 1];
                }
            }
        }

        segments
    }

    fn create_segment(
        ts: &OhlcvTimeSeries,
        start: usize,
        end: usize,
        prev_end_idx: usize,
        prev_end_ts: i64,
        is_first: bool,
        bounds: (f64, f64),
    ) -> DisplaySegment {
        let start_ts = ts.timestamps[start];
        let end_ts = ts.timestamps[end - 1]; 
        
        let (reason, duration_str) = if is_first {
            (GapReason::None, String::new())
        } else {
            let time_gap = start_ts - prev_end_ts;
            
            // LOGIC:
            // 1. If indices are contiguous, it's a Source Gap.
            // 2. If indices are skipped, check prices of the skipped candles.
            let reason = if start == prev_end_idx {
                GapReason::MissingSourceData
            } else {
                // Determine direction
                // We check the first excluded candle to see where it was.
                // (prev_end_idx is the index of the first excluded candle)
                if prev_end_idx < ts.low_prices.len() {
                    let low = ts.low_prices[prev_end_idx];
                    let high = ts.high_prices[prev_end_idx];
                    let (min_ph, max_ph) = bounds;
                    
                    if low > max_ph {
                        GapReason::PriceAbovePH
                    } else if high < min_ph {
                        GapReason::PriceBelowPH
                    } else {
                        GapReason::PriceMixed
                    }
                } else {
                    GapReason::PriceMixed
                }
            };
            
            (reason, time_utils::format_duration(time_gap))
        };

        DisplaySegment {
            start_idx: start,
            end_idx: end,
            start_ts,
            end_ts,
            candle_count: end - start,
            gap_reason: reason,
            gap_duration_str: duration_str,
        }
    }

}