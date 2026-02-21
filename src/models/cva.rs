use {
    crate::{
        config::{HighPrice, LowPrice, PhPct, Price, PriceRange, VolatilityPct},
        utils::MS_IN_D,
    },
    serde::{Deserialize, Serialize},
    std::fmt,
};

pub(crate) const PRICE_RECALC_THRESHOLD_PCT: PhPct = PhPct::new(0.01);
pub(crate) const MIN_CANDLES_FOR_ANALYSIS: usize = 250;
pub(crate) const SEGMENT_MERGE_TOLERANCE_MS: i64 = MS_IN_D;

/// Lean CVA results containing only actively used metrics
/// Memory footprint: ~3.2KB per 100 zones vs 14.4KB with full CVAResults
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct CVACore {
    // Active metrics (volume-weighted)
    pub candle_bodies_vw: Vec<f64>, // Mapped to FullCandleTVW

    pub low_wick_counts: Vec<f64>,
    pub high_wick_counts: Vec<f64>,

    pub total_candles: usize,

    pub included_ranges: Vec<(usize, usize)>,

    // Metadata
    pub pair_name: String,
    pub price_range: PriceRange<Price>,
    pub zone_count: usize,

    // Metadata fields required by pair_analysis.rs and ui_plot_view.rs
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub time_decay_factor: f64,

    // NEW METRICS
    pub relevant_candle_count: usize, // Number of candles inside the horizon
    pub interval_ms: i64,             // e.g. 3600000 for 1h
    pub volatility_pct: VolatilityPct, // Average (High-Low)/Close % for relevant candles
}

/// Score types for the lean CVA model
#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize, strum_macros::EnumIter,
)]
pub(crate) enum ScoreType {
    #[default]
    FullCandleTVW, // Sticky (Volume * Time)
    LowWickCount,  // Reversal (Count * Time) - Renamed from LowWickVW
    HighWickCount, // Reversal (Count * Time) - Renamed from HighWickVW
}

impl fmt::Display for ScoreType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScoreType::FullCandleTVW => write!(f, "Full Candle Temporal-Volume Weighted"),
            ScoreType::LowWickCount => write!(f, "Low Wick Count (Rejection Prob. Numerator)"),
            ScoreType::HighWickCount => write!(f, "High Wick Count (Rejection Prob. Numerator)"),
        }
    }
}

impl CVACore {
    pub(crate) fn get_scores_ref(&self, st: ScoreType) -> &Vec<f64> {
        match st {
            ScoreType::FullCandleTVW => &self.candle_bodies_vw,
            ScoreType::LowWickCount => &self.low_wick_counts,
            ScoreType::HighWickCount => &self.high_wick_counts,
        }
    }

    fn get_scores_mut_ref(&mut self, st: ScoreType) -> &mut Vec<f64> {
        match st {
            ScoreType::FullCandleTVW => &mut self.candle_bodies_vw,
            ScoreType::LowWickCount => &mut self.low_wick_counts,
            ScoreType::HighWickCount => &mut self.high_wick_counts,
        }
    }

    /// Applies a score to a range of zones WITHOUT diluting it.
    /// Used for: Wicks / Rejection / Presence.
    /// If a wick covers 5 zones, all 5 zones get the full rejection score.
    pub(crate) fn apply_rejection_impact(
        &mut self,
        st: ScoreType,
        start_range: Price,
        end_range: Price,
        score_to_apply: f64,
    ) {
        // Safety: Zero width implies no range to score
        if (start_range - end_range).abs() < f64::EPSILON {
            return;
        }

        let range_copy = self.price_range.clone();
        let num_chunks = range_copy.count_intersecting_chunks(start_range, end_range);

        if num_chunks == 0 {
            return;
        }

        // FLAT LOGIC: Do NOT divide by num_chunks.
        // We apply the full 'score_to_apply' to every bin touched.
        let start_chunk = range_copy.chunk_index(start_range);
        let scores = self.get_scores_mut_ref(st);

        scores
            .iter_mut()
            .enumerate()
            .skip(start_chunk)
            .take(num_chunks)
            .for_each(|(_, score)| {
                *score += score_to_apply;
            });
    }

    pub(crate) fn distribute_conserved_volume(
        &mut self,
        st: ScoreType,
        start_range: Price,
        end_range: Price,
        score_to_spread: f64,
    ) {
        if start_range == end_range {
            return;
        }

        let range_copy = self.price_range.clone();
        let num_chunks = range_copy.count_intersecting_chunks(start_range, end_range);

        if num_chunks == 0 {
            log::warn!(
                "Warning: num_chunks is 0 for range [{}, {}]. Skipping.",
                start_range,
                end_range
            );
            return;
        }

        // Density Logic: Divide score by number of zones covered
        let quantity_per_zone = score_to_spread / (num_chunks as f64);
        let start_chunk = range_copy.chunk_index(start_range);
        let scores = self.get_scores_mut_ref(st);

        scores
            .iter_mut()
            .enumerate()
            .skip(start_chunk)
            .take(num_chunks)
            .for_each(|(_, count)| {
                *count += quantity_per_zone;
            });
    }

    pub(crate) fn new(
        min_price: LowPrice,
        max_price: HighPrice,
        zone_count: usize,
        pair_name: String,
        time_decay_factor: f64,
        total_candles: usize,
        relevant_candle_count: usize,
        interval_ms: i64,
        volatility_pct: VolatilityPct,
    ) -> Self {
        let price_range: PriceRange<Price> =
            PriceRange::new(min_price.into(), max_price.into(), zone_count);
        let n_slices = price_range.n_chunks;

        CVACore {
            candle_bodies_vw: vec![0.0; n_slices],
            low_wick_counts: vec![0.0; n_slices],
            high_wick_counts: vec![0.0; n_slices],
            pair_name,
            price_range,
            zone_count,
            total_candles,
            relevant_candle_count,
            interval_ms,
            volatility_pct,
            included_ranges: Vec::new(),
            start_timestamp_ms: 0,
            end_timestamp_ms: 0,
            time_decay_factor,
        }
    }
}
