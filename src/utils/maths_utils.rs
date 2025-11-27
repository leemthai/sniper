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

    #[allow(dead_code)]
    pub fn min_max(&self) -> (f64, f64) {
        (self.start_range, self.end_range)
    }

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

    pub fn range_length(&self) -> f64 {
        self.end_range - self.start_range
    }

    pub fn chunk_size(&self) -> f64 {
        self.range_length() / (self.n_chunks as f64)
    }

    pub fn chunk_index(&self, value: f64) -> usize {
        let index = (value - self.start_range) / self.chunk_size();
        let chunk_index = index as usize;

        // Clamping handles floating-point inaccuracies at the boundary.
        chunk_index.min(self.n_chunks - 1)
    }

    #[allow(dead_code)]
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
pub fn intervals(range_start: i64, range_end: i64, interval: i64) -> i64 {
    debug_assert_eq!((range_end - range_start) % interval, 0);
    ((range_end - range_start) / interval) + 1
}

/// In which interval is `value`
pub fn index_into_range(range_start: i64, value: i64, range_interval: i64) -> i64 {
    debug_assert_eq!((value - range_start) % range_interval, 0);
    (value - range_start) / range_interval
}

pub fn get_max(vec: &[f64]) -> f64 {
    let max_index: usize = vec.argmax();
    vec[max_index]
}

pub fn get_min(vec: &[f64]) -> f64 {
    let max_index: usize = vec.argmin();
    vec[max_index]
}

#[allow(dead_code)]
pub fn get_min_max(vec: &[f64]) -> (f64, f64) {
    (get_min(vec), get_max(vec))
}

// Normalizes a vector of (positive) f64 to 0.0 to 1.0. Guarantees largest value is 1.0
// Smallest output value will be 0.0 iff smallest input value = 0.0
// Name: `Max normalization`, `Max-Abs normalization`, or `L∞ normalization`
#[allow(dead_code)]
pub fn normalize_max(vec: &[f64]) -> Vec<f64> {
    let max_value = get_max(vec);

    // If the largest value is 0 or non-positive, scaling may result in NaNs or -1.0
    // for all elements. For this example, we simply return.
    // TEMP this won't happen ! so make debug only
    #[cfg(debug_assertions)]
    {
        if max_value <= 0.0 {
            // In a real application, you might panic here or log an error
            // depending on your specific requirements.
            log::error!("Warning: max_value is <= 0.0. Returning original data.");
            return vec.to_vec();
        }
    }

    // Use a match expression to handle the non-positive case in release builds,
    // otherwise proceed with the normalization.
    match max_value {
        val if val <= 0.0 => vec.to_vec(),
        val => vec.iter().map(|&x| x / val).collect(),
    }
}

/// Normalizes a vector in-place using the L1 norm (Manhattan norm),
/// so that the sum of the absolute values of its components is 1.0.
#[allow(dead_code)]
pub fn normalize_manhattan(vec: &[f64]) -> Vec<f64> {
    let mut sum_of_absolute_values = 0.0;

    // Calculate the sum of absolute values (the L1 norm)
    for x in vec.iter() {
        sum_of_absolute_values += x.abs();
    }

    // Handle the edge case where the L1 norm is 0 to avoid division by zero.
    if sum_of_absolute_values == 0.0 {
        return vec.to_vec();
    }

    // Divide each element by the L1 norm
    // Divide each element by the L1 norm using iteration and collect into a new Vec
    vec.iter().map(|&x| x / sum_of_absolute_values).collect()
}

// Euclidean normalization or L2 norm.
#[allow(dead_code)]
fn euclidean_normalization(vec: &mut [f64]) {
    // Probably won't use this because it ensures the sum of squares of all the values add up to 1
    // 1. Calculate the sum of squares.
    let sum_of_squares = vec.iter().map(|x| x * x).sum::<f64>();

    // 2. Take the square root of the sum of squares to get the Euclidean norm.
    let euclidean_norm = sum_of_squares.sqrt();

    // 3. Divide each element by the Euclidean norm.
    // We must handle the case where the norm is zero to avoid division by zero.
    if euclidean_norm > 0.0 {
        for x in vec.iter_mut() {
            *x /= euclidean_norm;
        }
    }
}
