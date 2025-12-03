// use crate::models::cva::{CVACore, ScoreType};


/// Represents a clustered "Island" of activity.
#[derive(Debug, Clone)]
pub struct TargetZone {
    /// The starting index of this zone (inclusive)
    pub start_idx: usize,
    /// The ending index of this zone (inclusive)
    pub end_idx: usize,
    /// The Sum of all scores in this cluster (Mass/Strength)
    pub strength_mass: f64,
    /// The highest single score within this cluster
    pub peak_score: f64,
    /// The weighted center index (e.g. 45.3) - useful for plotting the "gravity center"
    pub center_of_mass: f64,
}

/// Identifies target zones using the "Islands" strategy (Threshold + Clustering).
/// 
/// 1. Filters all zones that meet the `threshold`.
/// 2. Clusters them together if they are within `max_gap` of each other.
/// 3. Computes the mass and center of gravity for each cluster.
pub fn find_target_zones(scores: &[f64], threshold: f64, max_gap: usize) -> Vec<TargetZone> {
    if scores.is_empty() {
        return Vec::new();
    }

    // Step 1: Identify all "Land" indices (scores above threshold)
    let valid_indices: Vec<usize> = scores
        .iter()
        .enumerate()
        .filter(|&(_, &score)| score >= threshold)
        .map(|(i, _)| i)
        .collect();

    if valid_indices.is_empty() {
        return Vec::new();
    }

    let mut targets = Vec::new();
    let mut cluster_start = valid_indices[0];
    let mut prev_idx = valid_indices[0];

    // Helper to finalize a cluster
    let mut finalize_cluster = |start: usize, end: usize| {
        let mut sum_score = 0.0;
        let mut sum_weighted_index = 0.0;
        let mut max_score = 0.0;

        // Iterate inclusive range [start, end]
        for i in start..=end {
            // Safety check although indices come from bounds
            if let Some(&s) = scores.get(i) {
                sum_score += s;
                sum_weighted_index += i as f64 * s;
                if s > max_score {
                    max_score = s;
                }
            }
        }

        let com = if sum_score > 0.0 {
            sum_weighted_index / sum_score
        } else {
            (start + end) as f64 / 2.0
        };

        targets.push(TargetZone {
            start_idx: start,
            end_idx: end,
            strength_mass: sum_score,
            peak_score: max_score,
            center_of_mass: com,
        });
    };

    // Step 2: Cluster indices based on max_gap
    for &idx in valid_indices.iter().skip(1) {
        // If the distance to the previous index is greater than gap + 1, the bridge breaks.
        // e.g. indices [2, 4] with max_gap 1. 4 - 2 = 2. (gap is 1). <= 2. Bridge holds.
        // e.g. indices [2, 5] with max_gap 1. 5 - 2 = 3. Bridge breaks.
        if idx - prev_idx > max_gap + 1 {
            // Finalize previous cluster
            finalize_cluster(cluster_start, prev_idx);
            // Start new cluster
            cluster_start = idx;
        }
        prev_idx = idx;
    }

    // Finalize the last cluster
    finalize_cluster(cluster_start, prev_idx);

    targets
}

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
