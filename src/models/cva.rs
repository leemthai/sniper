use crate::utils::maths_utils::RangeF64;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Lean CVA results containing only actively used metrics
/// Memory footprint: ~3.2KB per 100 zones vs 14.4KB with full CVAResults
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CVACore {
    // Active metrics (volume-weighted)
    pub candle_bodies_vw: Vec<f64>, // Sticky/slippy zone detection
    pub quote_volumes: Vec<f64>,
    pub low_wicks_vw: Vec<f64>,
    pub high_wicks_vw: Vec<f64>,

    // Metadata
    pub price_range: RangeF64,
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub zone_count: usize,
    pub pair_name: String,
    pub time_decay_factor: f64,
}

/// Score types for the lean CVA model
#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize, strum_macros::EnumIter,
)]
pub enum ScoreType {
    #[default]
    CandleBodyVW, // Volume-weighted
    LowWickVW,
    HighWickVW,
    QuoteVolume,
}

impl fmt::Display for ScoreType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScoreType::CandleBodyVW => write!(f, "Candle Bodies Volume Weighted"),
            ScoreType::LowWickVW => write!(f, "Low Wick Volume Weighted (reject @ low)"),
            ScoreType::HighWickVW => write!(f, "High Wick Volume Weighted (reject @ high)"),
            ScoreType::QuoteVolume => write!(f, "Quote Volume (transitions)"),
        }
    }
}

impl CVACore {
    pub fn get_scores_ref(&self, st: ScoreType) -> &Vec<f64> {
        match st {
            ScoreType::CandleBodyVW => &self.candle_bodies_vw,
            ScoreType::LowWickVW => &self.low_wicks_vw,
            ScoreType::HighWickVW => &self.high_wicks_vw,
            ScoreType::QuoteVolume => &self.quote_volumes,
        }
    }

    fn get_scores_mut_ref(&mut self, st: ScoreType) -> &mut Vec<f64> {
        match st {
            ScoreType::CandleBodyVW => &mut self.candle_bodies_vw,
            ScoreType::LowWickVW => &mut self.low_wicks_vw,
            ScoreType::HighWickVW => &mut self.high_wicks_vw,
            ScoreType::QuoteVolume => &mut self.quote_volumes,
        }
    }

    #[allow(dead_code)] // May be needed for custom scoring strategies
    pub fn increase_score_one_zone_weighted(&mut self, st: ScoreType, price: f64, weight: f64) {
        let range_copy = self.price_range.clone();
        let index = range_copy.chunk_index(price);
        let scores = self.get_scores_mut_ref(st);
        scores[index] += weight;
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
            log::error!(
                "Warning: num_chunks is 0 for range [{}, {}]. Skipping.",
                start_range,
                end_range
            );
            return;
        }

        let start_chunk = range_copy.chunk_index(start_range);
        let quantity_per_zone = score_to_spread / (num_chunks as f64);
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

    pub fn new(
        start_range: f64,
        end_range: f64,
        n_chunks: usize,
        pair_name: String,
        time_decay_factor: f64,
    ) -> Self {
        let price_range = RangeF64 {
            start_range,
            end_range,
            n_chunks,
        };
        let n_slices = price_range.n_chunks();
        Self {
            candle_bodies_vw: vec![0.0; n_slices],
            low_wicks_vw: vec![0.0; n_slices],
            high_wicks_vw: vec![0.0; n_slices],
            quote_volumes: vec![0.0; n_slices],
            price_range,
            start_timestamp_ms: 0,
            end_timestamp_ms: 0,
            zone_count: n_chunks,
            pair_name,
            time_decay_factor,
        }
    }
}
