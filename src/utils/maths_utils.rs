use argminmax::ArgMinMax;
use std::cmp::{max, min};
use std::f64;

#[derive(serde::Deserialize, serde::Serialize, Default, Debug, Clone)]
pub struct RangeF64 {
    pub start_range: f64,
    pub end_range: f64,
    pub n_chunks: usize,
}

impl RangeF64 {
    #[inline]
    pub fn n_chunks(&self) -> usize {
        self.n_chunks
    }

    pub fn new(start_range: f64, end_range: f64, n_chunks: usize) -> Self {
        Self {
            start_range,
            end_range,
            n_chunks,
        }
    }

    #[inline]
    pub fn min_max(&self) -> (f64, f64) {
        (self.start_range, self.end_range)
    }

    #[inline]
    pub fn count_intersecting_chunks(&self, mut x_low: f64, mut x_high: f64) -> usize {
        // Swap the values over if necessary
        if x_high < x_low {
            (x_low, x_high) = (x_high, x_low);
        }
        // Determine the indices of the first and last chunk intersected.
        // We use min and max to ensure the indices are within the valid range.
        let first_chunk_index = max(
            0,
            ((x_low - self.start_range) / self.chunk_size()).floor() as isize,
        );
        let last_chunk_index = min(
            (self.n_chunks - 1) as isize,
            ((x_high - self.start_range) / self.chunk_size()).floor() as isize,
        );

        // If the ranges don't overlap, return 0.
        // This can happen if `last_chunk_index < first_chunk_index`.
        // TEMP can this really happen? just put on for debug and find out
        #[cfg(debug_assertions)]
        if last_chunk_index < first_chunk_index {
            return 0;
        }
        // The number of intersecting chunks is inclusive of both ends.
        (last_chunk_index - first_chunk_index + 1) as usize
    }

    #[inline]
    pub fn range_length(&self) -> f64 {
        self.end_range - self.start_range
    }

    #[inline]
    pub fn chunk_size(&self) -> f64 {
        self.range_length() / (self.n_chunks as f64)
    }

    #[inline]
    pub fn chunk_index(&self, value: f64) -> usize {
        let index = (value - self.start_range) / self.chunk_size();
        let chunk_index = index as usize;

        // Clamping handles floating-point inaccuracies at the boundary.
        chunk_index.min(self.n_chunks - 1)
    }

    #[inline]
    pub fn chunk_bounds(&self, chunk_index: usize) -> (f64, f64) {
        debug_assert!(chunk_index < self.n_chunks);
        let lower_bound = self.start_range + chunk_index as f64 * self.chunk_size();
        let upper_bound = self.start_range + (chunk_index + 1) as f64 * self.chunk_size();
        (lower_bound, upper_bound)
    }
}

/// Given an interval size, how many intervals total in a given range,
/// This assumes the range is exclusive, and hence why we need to add 1
/// i.e `range_end` is start of the last interval, not the end
#[inline]
pub fn intervals(range_start: i64, range_end: i64, interval: i64) -> i64 {
    debug_assert_eq!((range_end - range_start) % interval, 0);
    ((range_end - range_start) / interval) + 1
}

/// In which interval is `value`
#[inline]
pub fn index_into_range(range_start: i64, value: i64, range_interval: i64) -> i64 {
    debug_assert_eq!((value - range_start) % range_interval, 0);
    (value - range_start) / range_interval
}

#[inline]
pub fn get_max(vec: &[f64]) -> f64 {
    let max_index: usize = vec.argmax();
    vec[max_index]
}

#[inline]
pub fn get_min(vec: &[f64]) -> f64 {
    let max_index: usize = vec.argmin();
    vec[max_index]
}

// Normalizes a vector of (positive) f64 to 0.0 to 1.0. Guarantees largest value is 1.0
// Smallest output value will be 0.0 iff smallest input value = 0.0
// Name: `Max normalization`, `Max-Abs normalization`, or `L∞ normalization`
#[inline]
pub fn normalize_max(vec: &[f64]) -> Vec<f64> {
    let max_value = get_max(vec);

    // If the largest value is 0 or non-positive, scaling may result in NaNs or -1.0
    // for all elements. For this example, we simply return.
    if max_value <= f64::EPSILON {
        // In a real application, you might panic here or log an error
        // depending on your specific requirements.
        // log::warn!("Warning: max_value is <= 0.0. Returning original data.");
        return vec.to_vec();
    }

    // Use a match expression to handle the non-positive case in release builds,
    // otherwise proceed with the normalization.
    match max_value {
        val if val <= 0.0 => vec.to_vec(),
        val => vec.iter().map(|&x| x / val).collect(),
    }
}

/// Applies a simple centered moving average to smooth the data.
/// window_size should be an odd number (e.g., 3, 5, 7).
#[inline]
pub fn smooth_data(data: &[f64], window_size: usize) -> Vec<f64> {
    if data.is_empty() {
        return Vec::new();
    }
    if window_size <= 1 {
        return data.to_vec();
    }

    let half_window = window_size / 2;
    let len = data.len();
    let mut smoothed = vec![0.0; len];

    for i in 0..len {
        let start = i.saturating_sub(half_window);
        let end = (i + half_window + 1).min(len);
        let sum: f64 = data[start..end].iter().sum();
        let count = end - start;
        smoothed[i] = sum / count as f64;
    }

    smoothed
}

#[inline]
pub fn calculate_stats(data: &[f64]) -> (f64, f64) {
    let count = data.len();
    if count == 0 {
        return (0.0, 0.0);
    }

    let sum: f64 = data.iter().sum();
    let mean = sum / count as f64;

    let variance: f64 = data.iter()
        .map(|value| {
            let diff = mean - *value;
            diff * diff
        })
        .sum::<f64>() / count as f64;

    (mean, variance.sqrt())
}