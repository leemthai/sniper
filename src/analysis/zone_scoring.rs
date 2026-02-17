/// Represents a clustered "Island" of activity.
#[derive(Debug, Clone)]
pub(crate) struct TargetZone {
    /// The starting index of this zone (inclusive)
    pub start_idx: usize,
    /// The ending index of this zone (inclusive)
    pub end_idx: usize,
}

/// Identifies target zones using the "Islands" strategy (Threshold + Clustering).
/// Filters all zones that meet the `threshold`.
/// Clusters them together if they are within `max_gap` of each other.
/// Computes the mass and center of gravity for each cluster.
pub(crate) fn find_target_zones(scores: &[f64], threshold: f64, max_gap: usize) -> Vec<TargetZone> {
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
        targets.push(TargetZone {
            start_idx: start,
            end_idx: end,
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
