use {argminmax::ArgMinMax, std::f64};

#[inline]
pub(crate) fn get_max(vec: &[f64]) -> f64 {
    let max_index: usize = vec.argmax();
    vec[max_index]
}

// Normalizes a vector of (positive) f64 to 0.0 to 1.0. Guarantees largest value is 1.0
// Smallest output value will be 0.0 iff smallest input value = 0.0
// Name: `Max normalization`, `Max-Abs normalization`, or `L∞ normalization`
pub(crate) fn normalize_max(vec: &[f64]) -> Vec<f64> {
    let max_value = get_max(vec);

    // If the largest value is 0 or non-positive, scaling may result in NaNs or -1.0
    // for all elements. For this example, we simply return.
    if max_value <= f64::EPSILON {
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
pub(crate) fn smooth_data(data: &[f64], window_size: usize) -> Vec<f64> {
    if data.is_empty() {
        return Vec::new();
    }
    if window_size <= 1 {
        return data.to_vec();
    }

    let half_window = window_size / 2;
    let len = data.len();
    let mut smoothed = vec![0.0; len];

    for (i, smoothed_val) in smoothed.iter_mut().enumerate() {
        let start = i.saturating_sub(half_window);
        let end = (i + half_window + 1).min(len);
        let sum: f64 = data[start..end].iter().sum();
        let count = end - start;
        *smoothed_val = sum / count as f64;
    }

    smoothed
}

#[inline]
pub(crate) fn mean_and_stddev(data: &[f64]) -> (f64, f64) {
    let count = data.len();
    if count == 0 {
        return (0.0, 0.0);
    }

    let sum: f64 = data.iter().sum();
    let mean = sum / count as f64;

    let variance: f64 = data
        .iter()
        .map(|value| {
            let diff = mean - *value;
            diff * diff
        })
        .sum::<f64>()
        / count as f64;

    (mean, variance.sqrt())
}

/// Linearly maps a value from one range to another while preserving its relative proportion.
pub(crate) fn remap(val: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    let t = (val - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}
