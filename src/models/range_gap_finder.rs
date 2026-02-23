use crate::{
    config::{HighPrice, LowPrice, Price},
    models::OhlcvTimeSeries,
    utils::TimeUtils,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GapReason {
    None,
    #[allow(unused)]
    PriceMismatch,
    MissingSourceData,
    PriceAbovePH,
    PriceBelowPH,
    PriceMixed,
}

#[derive(Debug, Clone)]
pub(crate) struct DisplaySegment {
    pub start_idx: usize,
    pub end_idx: usize,
    pub start_ts: i64,
    pub end_ts: i64,
    pub candle_count: usize,
    pub low_price: LowPrice,
    pub high_price: HighPrice,
    pub gap_reason: GapReason,
    pub gap_duration_str: String,
}

pub(crate) struct RangeGapFinder;

impl RangeGapFinder {
    /// Analyzes timeseries to produce display segments, merging gaps shorter than merge_tolerance_ms.
    /// Pass 1: Split on price horizon and source data gaps.
    /// Pass 2: Merge short price excursions within tolerance.
    pub(crate) fn analyze(
        timeseries: &OhlcvTimeSeries,
        ph_ranges: &[(usize, usize)],
        price_bounds: (Price, Price),
        merge_tolerance_ms: i64,
    ) -> Vec<DisplaySegment> {
        if ph_ranges.is_empty() || timeseries.timestamps.is_empty() {
            return Vec::new();
        }

        let interval_ms = timeseries.pair_interval.interval_ms;
        let source_gap_tolerance = (interval_ms as f64 * 1.1) as i64;

        // PASS 1: Generate raw segments
        let mut raw_segments = Vec::new();
        let mut prev_segment_end_idx = 0;
        let mut prev_segment_end_ts = 0;
        let mut first_segment = true;

        for &(range_start, range_end) in ph_ranges {
            if range_start >= timeseries.timestamps.len() {
                continue;
            }
            let safe_end = range_end.min(timeseries.timestamps.len());
            let mut current_sub_start = range_start;

            for i in range_start..safe_end {
                let current_ts = timeseries.timestamps[i];

                if i + 1 < safe_end {
                    let next_ts = timeseries.timestamps[i + 1];
                    let diff = next_ts - current_ts;

                    if diff > source_gap_tolerance {
                        let sub_end = i + 1;
                        raw_segments.push(Self::create_segment(
                            timeseries,
                            current_sub_start,
                            sub_end,
                            prev_segment_end_idx,
                            prev_segment_end_ts,
                            first_segment,
                            price_bounds,
                        ));

                        first_segment = false;
                        prev_segment_end_idx = sub_end;
                        prev_segment_end_ts = timeseries.timestamps[sub_end - 1];
                        current_sub_start = i + 1;
                    }
                }
            }

            if current_sub_start < safe_end {
                raw_segments.push(Self::create_segment(
                    timeseries,
                    current_sub_start,
                    safe_end,
                    prev_segment_end_idx,
                    prev_segment_end_ts,
                    first_segment,
                    price_bounds,
                ));
                first_segment = false;
                prev_segment_end_idx = safe_end;
                if safe_end > 0 {
                    prev_segment_end_ts = timeseries.timestamps[safe_end - 1];
                }
            }
        }

        // PASS 2: Merge small price gaps
        if raw_segments.is_empty() {
            return vec![];
        }

        let mut merged_segments = Vec::new();
        let mut current = raw_segments[0].clone();

        for next in raw_segments.into_iter().skip(1) {
            let gap_duration = next.start_ts - current.end_ts;
            let is_source_hole = matches!(next.gap_reason, GapReason::MissingSourceData);

            if !is_source_hole && gap_duration <= merge_tolerance_ms {
                // Merge: price excursion was short enough to ignore
                let skipped_count = next.start_idx.saturating_sub(current.end_idx);

                // Include excursion candles in bounds
                for i in current.end_idx..next.start_idx {
                    let l = timeseries.low_prices[i];
                    let h = timeseries.high_prices[i];
                    if l < current.low_price {
                        current.low_price = l;
                    }
                    if h > current.high_price {
                        current.high_price = h;
                    }
                }

                // Merge next segment's bounds
                if next.low_price < current.low_price {
                    current.low_price = next.low_price;
                }
                if next.high_price > current.high_price {
                    current.high_price = next.high_price;
                }

                current.end_idx = next.end_idx;
                current.end_ts = next.end_ts;
                current.candle_count += next.candle_count + skipped_count;
            } else {
                merged_segments.push(current);
                current = next;
            }
        }
        merged_segments.push(current);

        merged_segments
    }

    fn create_segment(
        ts: &OhlcvTimeSeries,
        start: usize,
        end: usize,
        prev_end_idx: usize,
        prev_end_ts: i64,
        is_first: bool,
        bounds: (Price, Price),
    ) -> DisplaySegment {
        let start_ts = ts.timestamps[start];
        let end_ts = ts.timestamps[end - 1];

        let (reason, duration_str) = if is_first {
            (GapReason::None, String::new())
        } else {
            let time_gap = start_ts - prev_end_ts;

            let reason = if start == prev_end_idx {
                GapReason::MissingSourceData
            } else if prev_end_idx < ts.low_prices.len() {
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
            };

            (reason, TimeUtils::format_duration(time_gap))
        };

        let mut seg_low = ts.low_prices[start];
        let mut seg_high = ts.high_prices[start];

        for i in start..end {
            let l = ts.low_prices[i];
            let h = ts.high_prices[i];
            if l < seg_low {
                seg_low = l;
            }
            if h > seg_high {
                seg_high = h;
            }
        }

        DisplaySegment {
            start_idx: start,
            end_idx: end,
            start_ts,
            end_ts,
            candle_count: end - start,
            gap_reason: reason,
            gap_duration_str: duration_str,
            low_price: seg_low,
            high_price: seg_high,
        }
    }
}
