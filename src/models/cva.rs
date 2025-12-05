use serde::{Deserialize, Serialize};
use std::fmt;

use crate::utils::maths_utils::RangeF64;

/// Lean CVA results containing only actively used metrics
/// Memory footprint: ~3.2KB per 100 zones vs 14.4KB with full CVAResults
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CVACore {
    // Active metrics (volume-weighted)
    pub candle_bodies_vw: Vec<f64>, // Mapped to FullCandleTVW
    
    pub low_wick_counts: Vec<f64>,  // Renamed from low_wicks_vw
    pub high_wick_counts: Vec<f64>, // Renamed from high_wicks_vw
    
    pub quote_volumes: Vec<f64>, // Keep for legacy/debug

    pub total_candles: usize,

    // Metadata
    pub pair_name: String,
    pub price_range: RangeF64,
    pub zone_count: usize,

    // Metadata fields required by pair_analysis.rs and ui_plot_view.rs
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub time_decay_factor: f64, 

}

/// Score types for the lean CVA model
#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize, strum_macros::EnumIter,
)]
pub enum ScoreType {
    #[default]
    FullCandleTVW, // Sticky (Volume * Time)
    LowWickCount,    // Reversal (Count * Time) - Renamed from LowWickVW
    HighWickCount,   // Reversal (Count * Time) - Renamed from HighWickVW
    QuoteVolume,     // Keep for debug/legacy
}

impl fmt::Display for ScoreType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScoreType::FullCandleTVW => write!(f, "Full Candle Temporal-Volume Weighted"),
            ScoreType::LowWickCount => write!(f, "Low Wick Count (Rejection Prob. Numerator)"),
            ScoreType::HighWickCount => write!(f, "High Wick Count (Rejection Prob. Numerator)"),
            ScoreType::QuoteVolume => write!(f, "Quote Volume (transitions)"),
        }
    }
}

impl CVACore {
    pub fn get_scores_ref(&self, st: ScoreType) -> &Vec<f64> {
        match st {
            ScoreType::FullCandleTVW => &self.candle_bodies_vw,
            ScoreType::LowWickCount => &self.low_wick_counts,
            ScoreType::HighWickCount => &self.high_wick_counts,
            ScoreType::QuoteVolume => &self.quote_volumes,
        }
    }

    fn get_scores_mut_ref(&mut self, st: ScoreType) -> &mut Vec<f64> {
        match st {
            ScoreType::FullCandleTVW => &mut self.candle_bodies_vw,
            ScoreType::LowWickCount => &mut self.low_wick_counts,
            ScoreType::HighWickCount => &mut self.high_wick_counts,
            ScoreType::QuoteVolume => &mut self.quote_volumes,
        }
    }

    // Updated helper to use new enum variants
    #[allow(dead_code)]
    pub fn increase_score_one_zone_weighted(&mut self, st: ScoreType, price: f64, weight: f64) {
        let range_copy = self.price_range.clone();
        let index = range_copy.chunk_index(price);
        let scores = self.get_scores_mut_ref(st);
        if index < scores.len() {
            scores[index] += weight;
        }
    }


    pub fn increase_score_multi_zones_spread(
        &mut self,
        st: ScoreType,
        start_range: f64,
        end_range: f64,
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

    // Updated Constructor to match src/models/timeseries.rs usage
    pub fn new(
        min_price: f64,
        max_price: f64,
        zone_count: usize,
        pair_name: String,
        time_decay_factor: f64,
        total_candles: usize,
    ) -> Self {
        let price_range = RangeF64::new(min_price, max_price, zone_count);
        let n_slices = price_range.n_chunks();

        CVACore {
            candle_bodies_vw: vec![0.0; n_slices],
            low_wick_counts: vec![0.0; n_slices],
            high_wick_counts: vec![0.0; n_slices],
            quote_volumes: vec![0.0; n_slices],
            pair_name,
            price_range,
            zone_count,
            total_candles,
            start_timestamp_ms: 0,
            end_timestamp_ms: 0,
            time_decay_factor,
        }
    }
}
