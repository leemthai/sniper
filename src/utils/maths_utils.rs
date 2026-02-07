use argminmax::ArgMinMax;
use std::f64;
use std::time::Duration;

/// Formats a float to occupy EXACTLY `width` characters.
/// Adjusts decimal precision automatically.
/// Returns None if the integer part is too large to fit.
///
/// Examples (width 6):
/// 0.002   -> "0.0020"
/// 12.1    -> "12.100"
/// 143.032 -> "143.03"
/// 100000  -> "100000"
/// 9999999 -> None
// pub(crate) fn format_fixed_chars(val: f64, width: usize) -> Option<String> {
//     if width == 0 { return None; }

//     let is_neg = val < 0.0;
//     let abs_val = val.abs();

//     // 1. Calculate available width for the number itself (excluding sign)
//     let content_width = if is_neg {
//         if width < 2 { return None; } // No room for "-" and a digit
//         width - 1
//     } else {
//         width
//     };

//     // 2. Calculate Integer Length
//     // Log10 gives us the magnitude. 
//     // e.g. log10(9) = 0.95 -> floor 0 -> len 1.
//     // e.g. log10(10) = 1.0 -> floor 1 -> len 2.
//     // Handle 0.0 explicitly to avoid -inf.
//     let int_len = if abs_val < 1.0 {
//         1
//     } else {
//         abs_val.log10().floor() as usize + 1
//     };

//     if int_len > content_width {
//         return None; // Integer part too big to fit
//     }

//     // 3. Calculate Precision
//     // We need room for: Integer Part + Dot + Fraction
//     // Fraction = Available - Integer - 1 (Dot)
//     // If int_len == content_width, we have 0 precision (and no dot).
//     let precision = if int_len >= content_width {
//         0
//     } else {
//         content_width - int_len - 1
//     };

//     // 4. Format
//     let s = format!("{:.1$}", abs_val, precision);

//     // 5. Check for Overflow (Rounding Up)
//     // e.g. val=9.9, width=2. 
//     // int_len=1. prec=0. fmt="10". len=2. OK.
//     // e.g. val=9.95, width=3. 
//     // int_len=1. prec=1. fmt="10.0". len=4 (Overflow).
//     // In overflow case, we reduce precision by 1 or just return the integer if it fits.
    
//     let final_str = if s.len() > content_width {
//         // Rounding caused digit increase (9.9 -> 10.0).
//         // Try reducing precision (usually to 0 or by 1).
//         // If we were at prec 0 and overflowed (99 -> 100 in width 2), we fail.
//         if precision > 0 {
//              let s_retry = format!("{:.1$}", abs_val, precision - 1);
//              if s_retry.len() <= content_width {
//                  s_retry
//              } else {
//                  return None; // Still doesn't fit
//              }
//         } else {
//             return None; // Integer overflowed width
//         }
//     } else {
//         s
//     };

//     // 6. Assemble and Pad
//     // We pad to the LEFT to ensure fixed width (right-aligned numbers).
//     let raw_output = format!("{}{}", if is_neg { "-" } else { "" }, final_str);
//     Some(format!("{:>1$}", raw_output, width))
// }


/// Converts a Duration into a specific number of candles based on the interval.
pub(crate) fn duration_to_candles(duration: Duration, interval_ms: i64) -> usize {
    if interval_ms <= 0 { return 0; }
    (duration.as_millis() as i64 / interval_ms) as usize
}

#[inline]
pub(crate) fn get_max(vec: &[f64]) -> f64 {
    let max_index: usize = vec.argmax();
    vec[max_index]
}

// #[inline]
// pub(crate) fn get_min(vec: &[f64]) -> f64 {
//     let max_index: usize = vec.argmin();
//     vec[max_index]
// }

// Normalizes a vector of (positive) f64 to 0.0 to 1.0. Guarantees largest value is 1.0
// Smallest output value will be 0.0 iff smallest input value = 0.0
// Name: `Max normalization`, `Max-Abs normalization`, or `L∞ normalization`
#[inline]
pub(crate) fn normalize_max(vec: &[f64]) -> Vec<f64> {
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
pub fn mean_and_stddev(data: &[f64]) -> (f64, f64) {
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

/// Linearly maps a value from one range to another while preserving its relative proportion.
pub fn remap(val: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    let t = (val - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}