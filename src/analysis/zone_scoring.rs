// Zone scoring and combination strategies for identifying key price levels
#[cfg(debug_assertions)]
use crate::config::PRINT_ZONE_SCORING_FOR_PAIR;
use crate::models::cva::{CVACore, ScoreType};
use std::collections::HashSet;

/// Strategy for combining multiple normalized data sources into a single zone score
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CombinationStrategy {
    /// Simple arithmetic average of all scores
    /// Best for: Balanced consensus across indicators
    Average,

    /// Geometric mean: (a * b * c)^(1/n)
    /// Best for: Requiring moderate agreement across all indicators
    /// Penalizes low outliers more than Average
    GeometricMean,

    /// Product of all scores: a * b * c
    /// Best for: Requiring strong agreement across ALL indicators
    /// Heavily penalizes any low value (most conservative)
    Product,

    /// Weighted sum with custom weights per data source
    /// Best for: When certain indicators are more important
    /// Weights should sum to 1.0 for normalized output
    WeightedSum(Vec<f64>),

    /// Minimum score across all sources (weakest link)
    /// Best for: Ultra-conservative selection (zone must be strong in ALL indicators)
    Min,

    /// Maximum score across all sources (best signal)
    /// Best for: Optimistic selection (zone strong in ANY indicator)
    Max,
}

impl CombinationStrategy {
    /// Combine multiple scores for a single zone
    #[allow(dead_code)] // Used by ZoneScorer
    fn combine(&self, scores: &[f64]) -> f64 {
        if scores.is_empty() {
            return 0.0;
        }

        match self {
            CombinationStrategy::Average => scores.iter().sum::<f64>() / scores.len() as f64,

            CombinationStrategy::GeometricMean => {
                let product: f64 = scores.iter().product();
                product.powf(1.0 / scores.len() as f64)
            }

            CombinationStrategy::Product => scores.iter().product(),

            CombinationStrategy::WeightedSum(weights) => {
                if weights.len() != scores.len() {
                    log::error!(
                        "Warning: WeightedSum has {} weights but {} scores. Using average instead.",
                        weights.len(),
                        scores.len()
                    );
                    return scores.iter().sum::<f64>() / scores.len() as f64;
                }
                scores.iter().zip(weights).map(|(s, w)| s * w).sum()
            }

            CombinationStrategy::Min => scores.iter().copied().fold(f64::INFINITY, f64::min),

            CombinationStrategy::Max => scores.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        }
    }
}

/// Computes combined zone scores from multiple normalized data sources
#[allow(dead_code)] // Full API available for advanced zone analysis
pub struct ZoneScorer {
    data_sources: Vec<ScoreType>,
    strategy: CombinationStrategy,
}

#[allow(dead_code)] // Full API available for advanced zone analysis
impl ZoneScorer {
    /// Create a new zone scorer with specified data sources and combination strategy
    #[allow(dead_code)] // Part of ZoneScorer API
    pub fn new(data_sources: Vec<ScoreType>, strategy: CombinationStrategy) -> Self {
        Self {
            data_sources,
            strategy,
        }
    }

    /// Create a zone scorer from a set of score types (order doesn't matter unless using WeightedSum)
    #[allow(dead_code)] // Part of ZoneScorer API
    pub fn from_set(data_sources: HashSet<ScoreType>, strategy: CombinationStrategy) -> Self {
        let mut sources: Vec<ScoreType> = data_sources.into_iter().collect();
        sources.sort_by_key(|st| format!("{:?}", st)); // Deterministic ordering
        Self::new(sources, strategy)
    }

    /// Compute combined importance score for each zone
    /// Returns None if any data source is invalid
    #[allow(dead_code)] // Part of ZoneScorer API
    pub fn compute_scores(&self, cva_results: &CVACore) -> Option<Vec<f64>> {
        use crate::utils::maths_utils;

        // Extract normalized data from each source
        let all_data: Vec<Vec<f64>> = self
            .data_sources
            .iter()
            .filter_map(|&score_type| {
                let raw_data = cva_results.get_scores_ref(score_type);
                if raw_data.is_empty() {
                    None
                } else {
                    Some(maths_utils::normalize_max(raw_data))
                }
            })
            .collect();

        if all_data.is_empty() || all_data[0].is_empty() {
            return None;
        }

        let zone_count = all_data[0].len();
        let mut combined = vec![0.0; zone_count];

        // Combine scores for each zone
        for zone_idx in 0..zone_count {
            let scores: Vec<f64> = all_data.iter().map(|data| data[zone_idx]).collect();
            combined[zone_idx] = self.strategy.combine(&scores);
        }

        Some(combined)
    }

    /// Get the data sources used by this scorer
    #[allow(dead_code)] // Part of ZoneScorer API
    pub fn data_sources(&self) -> &[ScoreType] {
        &self.data_sources
    }

    /// Get the combination strategy
    #[allow(dead_code)]
    pub fn strategy(&self) -> &CombinationStrategy {
        &self.strategy
    }

    /// Check if this scorer uses multiple data sources
    #[allow(dead_code)] // Part of ZoneScorer API
    pub fn is_multi_source(&self) -> bool {
        self.data_sources.len() > 1
    }
}

/// Calculate gradient (rate of change) between adjacent zones
/// Returns vector of size scores.len() - 1
pub fn calculate_zone_gradient(zone_scores: &[f64]) -> Vec<f64> {
    if zone_scores.len() < 2 {
        return Vec::new();
    }

    zone_scores
        .windows(2)
        .map(|window| (window[1] - window[0]).abs())
        .collect()
}

/// Find zones with high activity and low gradient (hence sustained activity, not spikes)
/// Used for sticky zones (consolidation) and other sustained patterns
pub fn find_high_activity_zones_low_gradient(
    zone_scores: &[f64],
    top_percentile: f64,
    gradient_percentile: f64,
) -> Vec<usize> {
    if zone_scores.is_empty() {
        return Vec::new();
    }

    let gradients = calculate_zone_gradient(zone_scores);

    // Calculate score threshold (top X%)
    // let mut sorted_scores: Vec<f64> = zone_scores.iter().copied().collect();
    let mut sorted_scores: Vec<f64> = zone_scores.to_vec();

    sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let score_threshold_idx =
        ((sorted_scores.len() as f64 * top_percentile) as usize).min(sorted_scores.len() - 1);
    let score_threshold = sorted_scores[score_threshold_idx];

    // Calculate gradient threshold (low gradients only)
    let mut sorted_gradients = gradients.clone();
    sorted_gradients.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let gradient_threshold_idx = ((sorted_gradients.len() as f64 * gradient_percentile) as usize)
        .min(sorted_gradients.len().saturating_sub(1));
    let max_gradient = if !sorted_gradients.is_empty() {
        sorted_gradients[gradient_threshold_idx]
    } else {
        f64::INFINITY
    };

    zone_scores
        .iter()
        .enumerate()
        .filter(|(i, score)| {
            **score >= score_threshold
                && (
                    // Check gradient before this zone
                    (*i == 0 || gradients.get(i - 1).is_none_or(|&g| g <= max_gradient))
                    // Check gradient after this zone
                    && gradients.get(*i).is_none_or(|&g| g <= max_gradient)
                )
        })
        .map(|(i, _)| i)
        .collect()
}

/// Find zones with high activity but don't care about gradient to next zones
/// Because low wick and high wicks are often single candles with large wicks, so gradient doesn't matter
pub fn find_high_activity_zones(zone_scores: &[f64], top_percentile: f64) -> Vec<usize> {
    if zone_scores.is_empty() {
        return Vec::new();
    }

    // Calculate score threshold (top X%)
    let mut sorted_scores: Vec<f64> = zone_scores.to_vec();
    sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let score_threshold_idx =
        ((sorted_scores.len() as f64 * top_percentile) as usize).min(sorted_scores.len() - 1);
    let score_threshold = sorted_scores[score_threshold_idx];

    // Simply return all zones above the threshold
    // No gradient check - reversals are often sharp, single-candle events
    zone_scores
        .iter()
        .enumerate()
        .filter(|(_, score)| **score >= score_threshold)
        .map(|(i, _)| i)
        .collect()
}

/// Finds consolidation zones by detecting peaks and expanding around them
///
/// # Parameters
///
/// * `zone_scores` - Normalized zone scores (0.0 to 1.0)
/// * `min_peak_height` - Minimum absolute score for a zone to be considered a peak
/// * `min_prominence` - Minimum prominence (standing out from surroundings)
/// * `min_distance_fraction` - Minimum spacing as fraction of total zones (e.g., 0.08 = 8%)
/// * `expansion_threshold` - Max height difference for including zones near peaks
/// * `max_expansion_fraction` - Max expansion distance as fraction of total zones (e.g., 0.03 = 3%)
/// * `strength_tolerance` - Allow peaks within min_distance if ≥X% of nearby stronger peaks
///
/// # Example
///
/// ```ignore
/// // For 100 zones: min_distance = 8, max_expansion = 3
/// let zones = find_consolidation_zones_from_peaks(
///     &scores, 0.4, 0.05, 0.08, 0.05, 0.03, 0.9
/// );
///
/// // For 500 zones: min_distance = 40, max_expansion = 15
/// let zones = find_consolidation_zones_from_peaks(
///     &scores, 0.4, 0.05, 0.08, 0.05, 0.03, 0.9
/// );
/// ```
#[allow(clippy::too_many_arguments)]
pub fn find_consolidation_zones_from_peaks(
    zone_scores: &[f64],
    min_peak_height: f64,
    min_prominence: f64,
    min_distance_fraction: f64, // e.g., 0.08 = 8% of zones
    expansion_threshold: f64,
    max_expansion_fraction: f64, // e.g., 0.03 = 3% of zones
    strength_tolerance: f64,
    _pair_name: String,                // For debugging, really
    min_single_zone_gap_fill_pct: f64, // e.g. 0.8 = 80%
) -> Vec<usize> {
    let zone_count = zone_scores.len();
    let min_distance = (zone_count as f64 * min_distance_fraction).max(1.0) as usize;
    let max_expansion_distance = (zone_count as f64 * max_expansion_fraction).max(1.0) as usize;

    // Find ALL local maxima
    let mut fp = find_peaks::PeakFinder::new(zone_scores);
    fp.with_min_height(min_peak_height);
    fp.with_min_prominence(min_prominence);

    let raw_peaks = fp.find_peaks();

    // Extract (position, height) pairs and sort by strength
    let mut candidate_peaks: Vec<(usize, f64)> = raw_peaks
        .iter()
        .map(|p| (p.middle_position(), p.height.unwrap_or(0.0)))
        .collect();
    candidate_peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // New code 17/1//25 - Manually check boundaries for peaks that might have been missed
    // Left boundary (index 0)
    if zone_scores.len() >= 2 {
        let left_val = zone_scores[0];
        let right_val = zone_scores[1];
        if left_val >= min_peak_height && left_val >= right_val + min_prominence {
            // Calculate prominence for boundary peak
            let prominence = left_val - right_val;
            #[cfg(debug_assertions)]
            if !PRINT_ZONE_SCORING_FOR_PAIR.is_empty() && PRINT_ZONE_SCORING_FOR_PAIR == _pair_name
            {
                log::info!(
                    "{}: Adding left boundary peak at index 0 with prominence {}",
                    _pair_name,
                    prominence
                );
            }
            candidate_peaks.push((0, prominence));
        }
    }

    // Right boundary (index n-1)
    if zone_scores.len() >= 2 {
        let n = zone_scores.len();
        let left_val = zone_scores[n - 2];
        let right_val = zone_scores[n - 1];
        if right_val >= min_peak_height && right_val >= left_val + min_prominence {
            // Calculate prominence for boundary peak
            let prominence = right_val - left_val;
            #[cfg(debug_assertions)]
            if !PRINT_ZONE_SCORING_FOR_PAIR.is_empty() && PRINT_ZONE_SCORING_FOR_PAIR == _pair_name
            {
                log::info!(
                    "{}: Adding right boundary peak at index {} with prominence {}",
                    _pair_name,
                    n - 1,
                    prominence
                );
            }
            candidate_peaks.push((n - 1, prominence));
        }
    }

    // Remove duplicates (in case find_peaks already detected them)
    candidate_peaks.sort_by_key(|&(pos, _)| pos);
    candidate_peaks.dedup_by_key(|(pos, _)| *pos);

    candidate_peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // After dedup and sort:
    #[cfg(debug_assertions)]
    if !PRINT_ZONE_SCORING_FOR_PAIR.is_empty() && PRINT_ZONE_SCORING_FOR_PAIR == _pair_name {
        log::info!(
            "DEBUG: Candidate peaks after boundary check: {:?}",
            candidate_peaks
        );
    }

    // Strength-based selection
    let mut selected_peaks = Vec::new();
    for &(pos, height) in &candidate_peaks {
        // Find strongest selected peak within computed min_distance
        let strongest_nearby = selected_peaks
            .iter()
            .filter(|&&(selected_pos, _)| {
                let distance = pos.abs_diff(selected_pos);
                distance < min_distance
            })
            .map(|&(_, selected_height)| selected_height)
            .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap());

        let allow = if pos == 0 || pos == zone_scores.len() - 1 {
            true // Always allow boundary peaks
        } else {
            match strongest_nearby {
                None => true,
                Some(nearby_height) => height >= strength_tolerance * nearby_height,
            }
        };

        #[cfg(debug_assertions)]
        {
            let reason = if pos == 0 || pos == zone_scores.len() - 1 {
                "BOUNDARY - ALLOWED"
            } else {
                match strongest_nearby {
                    None => "NO NEARBY - ALLOWED",
                    Some(nearby) => {
                        if height >= strength_tolerance * nearby {
                            "STRONG ENOUGH - ALLOWED"
                        } else {
                            "TOO WEAK - REJECTED"
                        }
                    }
                }
            };
            if !PRINT_ZONE_SCORING_FOR_PAIR.is_empty() && PRINT_ZONE_SCORING_FOR_PAIR == _pair_name
            {
                log::info!(
                    "DEBUG: Peak at {} (prominence {:.6}): {}",
                    pos,
                    height,
                    reason
                );
            }
        }

        if allow {
            selected_peaks.push((pos, height));
        }
    }

    let peak_indices: Vec<usize> = selected_peaks.iter().map(|&(pos, _)| pos).collect();

    // Expansion logic
    let mut included_zones = std::collections::HashSet::new();
    for &peak_idx in &peak_indices {
        let peak_height = zone_scores[peak_idx];
        // included_zones.insert(peak_idx);

        let left_start = peak_idx.saturating_sub(max_expansion_distance);
        #[allow(clippy::needless_range_loop)]
        for i in left_start..peak_idx {
            if (peak_height - zone_scores[i]) <= expansion_threshold {
                included_zones.insert(i);
            } else {
                break;
            }
        }

        let right_end = (peak_idx + max_expansion_distance + 1).min(zone_scores.len());
        #[allow(clippy::needless_range_loop)]
        for i in peak_idx + 1..right_end {
            if (peak_height - zone_scores[i]) <= expansion_threshold {
                included_zones.insert(i);
            } else {
                break;
            }
        }

        // Now add the peak and debug
        let _before_expansion_count = included_zones.len();
        included_zones.insert(peak_idx); // Add the peak itself now
        let _after_expansion_count = included_zones.len();
        #[cfg(debug_assertions)]
        if !PRINT_ZONE_SCORING_FOR_PAIR.is_empty() && PRINT_ZONE_SCORING_FOR_PAIR == _pair_name {
            log::info!(
                "DEBUG: Peak at {} expanded zones: {} (pre-peak) -> {} (added {})",
                peak_idx,
                _before_expansion_count,
                _after_expansion_count,
                _after_expansion_count - _before_expansion_count
            );
        }
    }

    let mut result: Vec<usize> = included_zones.into_iter().collect();
    result.sort_unstable();
    // Post-processing: Fill single-zone gaps if the gap score is reasonably high
    let mut extended_result = result.clone();
    for window in result.windows(2) {
        if window[1] == window[0] + 2 {
            // Gap of exactly 1 zone
            let gap_idx = window[0] + 1;
            let left_peak = zone_scores[window[0]];
            let right_peak = zone_scores[window[1]];
            let avg_peaks = (left_peak + right_peak) / 2.0;
            if zone_scores[gap_idx] >= min_single_zone_gap_fill_pct * avg_peaks {
                // Threshold: 80% of average
                #[cfg(debug_assertions)]
                if !PRINT_ZONE_SCORING_FOR_PAIR.is_empty()
                    && PRINT_ZONE_SCORING_FOR_PAIR == _pair_name
                {
                    log::info!(
                        "{}: Filling gap at {} (score {:.3}) between peaks at {} ({:.3}) and {} ({:.3})",
                        _pair_name,
                        gap_idx,
                        zone_scores[gap_idx],
                        window[0],
                        left_peak,
                        window[1],
                        right_peak
                    );
                }
                extended_result.push(gap_idx);
            }
        }
    }
    extended_result.sort_unstable();
    extended_result.dedup();
    result = extended_result;

    result
}

/// Identify slippy zones - low score with low gradient (PERCENTILE VERSION)
/// bottom_percentile: e.g., 0.20 = bottom 20% of scores
/// gradient_percentile: e.g., 0.70 = zones with gradient in bottom 30%
pub fn find_low_activity_zones_low_gradient(
    zone_scores: &[f64],
    bottom_percentile: f64,
    gradient_percentile: f64,
) -> Vec<usize> {
    if zone_scores.is_empty() {
        return Vec::new();
    }

    let gradients = calculate_zone_gradient(zone_scores);

    // Calculate score threshold (bottom X%)
    // let mut sorted_scores: Vec<f64> = zone_scores.iter().copied().collect();
    let mut sorted_scores: Vec<f64> = zone_scores.to_vec();

    sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let score_threshold_idx =
        ((sorted_scores.len() as f64 * bottom_percentile) as usize).min(sorted_scores.len() - 1);
    let score_threshold = sorted_scores[score_threshold_idx];

    // Calculate gradient threshold (low gradients only)
    let mut sorted_gradients = gradients.clone();
    sorted_gradients.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let gradient_threshold_idx = ((sorted_gradients.len() as f64 * gradient_percentile) as usize)
        .min(sorted_gradients.len().saturating_sub(1));
    let max_gradient = if !sorted_gradients.is_empty() {
        sorted_gradients[gradient_threshold_idx]
    } else {
        f64::INFINITY
    };

    zone_scores
        .iter()
        .enumerate()
        .filter(|(i, score)| {
            **score <= score_threshold
                && (
                    // Check gradient before this zone
                    (*i == 0 || gradients.get(i - 1).is_none_or(|&g| g <= max_gradient))
                    // Check gradient after this zone
                    && gradients.get(*i).is_none_or(|&g| g <= max_gradient)
                )
        })
        .map(|(i, _)| i)
        .collect()
}
