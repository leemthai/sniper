use crate::config::PriceHorizonConfig;
use crate::config::{ANALYSIS, DEBUG_FLAGS};

use crate::data::timeseries::TimeSeriesCollection;

use crate::models::horizon_profile::{HorizonBucket, HorizonProfile};
use crate::models::timeseries::find_matching_ohlcv;

use crate::utils::time_utils::AppInstant;

pub fn generate_profile(
    pair: &str,
    timeseries_collection: &TimeSeriesCollection,
    current_price: f64,
    config_ref: &PriceHorizonConfig,
) -> HorizonProfile {
    let t_total_start = AppInstant::now();
    let mut profile = HorizonProfile::new();

    // Store validation data
    profile.base_price = current_price;
    profile.min_pct = config_ref.min_threshold_pct;
    profile.max_pct = config_ref.max_threshold_pct;

    // 1. Find Data
    let Some(ohlcv) = find_matching_ohlcv(
        &timeseries_collection.series_data,
        pair,
        ANALYSIS.interval_width_ms,
    )
    .ok() else {
        return profile;
    };

    let min_pct = config_ref.min_threshold_pct;
    let max_pct = config_ref.max_threshold_pct;
    let steps = config_ref.profiler_steps.max(100);

    // Pre-allocate buckets
    let step_size = (max_pct - min_pct) / steps as f64;
    for i in 0..=steps {
        let pct = min_pct + (i as f64 * step_size);
        profile.buckets.push(HorizonBucket {
            threshold_pct: pct,
            candle_count: 0,
            duration_days: 0.0,
            min_ts: i64::MAX,
            max_ts: i64::MIN,
        });
    }

    let total_candles = ohlcv.klines();
    if total_candles == 0 {
        return profile;
    }

    let mut frequency_map = vec![0usize; steps + 1];
    let mut min_ts_map = vec![i64::MAX; steps + 1];
    let mut max_ts_map = vec![i64::MIN; steps + 1];

    // --- PHASE 1: PREP (Minimal) ---
    // Just allocate results buffer. No data copying.
    let t_prep_start = AppInstant::now();
    let mut calculated_indices = vec![u32::MAX; total_candles];
    let t_prep = t_prep_start.elapsed();

    // --- PHASE 2: MATH (AVX-512 f64) ---
    let t_math_start = AppInstant::now();
    let mut _was_simd = false;

    // Stride is 8 for f64 (512 bits / 64 bits = 8)
    let stride = 8;
    let main_loop_len = total_candles - (total_candles % stride);

    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    if is_x86_feature_detected!("avx512f") {
        _was_simd = true;
        unsafe {
            use std::arch::x86_64::*;
            // Broadcast constants (f64)
            let cur_price_vec = _mm512_set1_pd(current_price);
            let inv_cur_price = _mm512_set1_pd(1.0 / current_price);

            let v_min_pct = _mm512_set1_pd(min_pct);
            let v_step_recip = _mm512_set1_pd(1.0 / step_size);
            let v_zero = _mm512_setzero_pd();
            let v_max_pct = _mm512_set1_pd(max_pct);

            // Mask/Blend Constant: 256-bit register of -1 (for 8 integers)
            let v_max_u32 = _mm256_set1_epi32(-1i32);

            // Direct pointers to existing memory
            let high_ptr = ohlcv.high_prices.as_ptr();
            let low_ptr = ohlcv.low_prices.as_ptr();

            for i in (0..main_loop_len).step_by(stride) {
                // Load 8 doubles from SoA layout
                let h = _mm512_loadu_pd(high_ptr.add(i));
                let l = _mm512_loadu_pd(low_ptr.add(i));

                // 1. Dist Below (f64)
                let diff_below = _mm512_sub_pd(cur_price_vec, h);
                let pct_below = _mm512_mul_pd(diff_below, inv_cur_price);

                // 2. Dist Above (f64)
                let diff_above = _mm512_sub_pd(l, cur_price_vec);
                let pct_above = _mm512_mul_pd(diff_above, inv_cur_price);

                // 3. Max(Below, Above, 0.0)
                let tmp = _mm512_max_pd(pct_below, pct_above);
                let required = _mm512_max_pd(tmp, v_zero);

                // 4. Compare & Index
                // _CMP_LE_OQ: Less-Equal, Ordered (non-NaN)
                // Returns an 8-bit mask (__mmask8)
                let mask_valid = _mm512_cmp_pd_mask(required, v_max_pct, _CMP_LE_OQ);

                // Calculate Bucket Index
                let relative = _mm512_sub_pd(required, v_min_pct);
                // Round scale 0x02 = Ceil
                let raw_idx_f = _mm512_roundscale_pd(_mm512_mul_pd(relative, v_step_recip), 0x02);

                // Compress 512-bit floats (8 doubles) -> 256-bit ints (8 i32s)
                let idx_u32 = _mm512_cvtpd_epu32(raw_idx_f);

                // Blend valid indices with -1 (MAX) based on mask
                let final_indices = _mm256_mask_blend_epi32(mask_valid, v_max_u32, idx_u32);

                // Store 256-bits (8 ints)
                _mm256_storeu_si256(
                    calculated_indices.as_mut_ptr().add(i) as *mut _,
                    final_indices,
                );
            }
        }
    }

    // --- SCALAR TAIL / FALLBACK ---
    // If SIMD ran, we pick up at `main_loop_len`. If not, we start at 0.
    let start_scalar = if _was_simd { main_loop_len } else { 0 };

    // Helper closure for scalar logic to ensure identical behavior
    let scalar_logic = |i: usize, dest: &mut [u32]| {
        let h = ohlcv.high_prices[i];
        let l = ohlcv.low_prices[i];

        let dist_below = if h < current_price {
            (current_price - h) / current_price
        } else {
            0.0
        };
        let dist_above = if l > current_price {
            (l - current_price) / current_price
        } else {
            0.0
        };
        let required = dist_below.max(dist_above);

        if required <= max_pct {
            let raw = ((required - min_pct) / step_size).ceil();
            let idx = if raw < 0.0 { 0 } else { raw as u32 };
            dest[i] = idx;
        } else {
            dest[i] = u32::MAX;
        }
    };

    // Run Scalar Logic on the Tail
    for i in start_scalar..total_candles {
        scalar_logic(i, &mut calculated_indices);
    }

    let t_math = t_math_start.elapsed();

    // --- PHASE 3: HISTOGRAM (Scatter) ---
    let t_agg_start = AppInstant::now();
    for i in 0..total_candles {
        let idx = calculated_indices[i];
        if idx != u32::MAX {
            let start_index = idx as usize;
            if start_index <= steps {
                frequency_map[start_index] += 1;
                // Direct access to timestamps vec
                let ts = ohlcv.timestamps[i];
                if ts < min_ts_map[start_index] {
                    min_ts_map[start_index] = ts;
                }
                if ts > max_ts_map[start_index] {
                    max_ts_map[start_index] = ts;
                }
            }
        }
    }

    // 4. Cumulative Sum
    let mut running_count = 0;
    let mut running_min_ts = i64::MAX;
    let mut running_max_ts = i64::MIN;

    for i in 0..=steps {
        running_count += frequency_map[i];
        if min_ts_map[i] != i64::MAX {
            if min_ts_map[i] < running_min_ts {
                running_min_ts = min_ts_map[i];
            }
            if max_ts_map[i] > running_max_ts {
                running_max_ts = max_ts_map[i];
            }
        }

        let bucket = &mut profile.buckets[i];
        bucket.candle_count = running_count;

        if running_count > 0 {
            let duration_ms = running_max_ts.saturating_sub(running_min_ts);
            bucket.duration_days = duration_ms as f64 / (1000.0 * 60.0 * 60.0 * 24.0);
            bucket.min_ts = running_min_ts;
            bucket.max_ts = running_max_ts;
        } else {
            bucket.duration_days = 0.0;
        }

        if running_count > profile.max_candle_count {
            profile.max_candle_count = running_count;
        }
    }
    let t_agg = t_agg_start.elapsed();
    let t_total = t_total_start.elapsed();

    // Manual trace_time logic: Check global flag before logging
    let t_threshold = 2000;
    if DEBUG_FLAGS.enable_perf_logging {
        if t_total.as_micros() > t_threshold {
            log::error!(
                "TRACE: HorizonProfiler [{}]: Total {:.2?} (Prep: {:.2?} | Math: {:.2?} | Agg: {:.2?}) (Threshold: {}ms)",
                pair,
                t_total,
                t_prep,
                t_math,
                t_agg,
                t_threshold,
            );
        }
    }

    profile
}
