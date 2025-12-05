use std::cmp::Eq;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};

use crate::config::ANALYSIS;

use crate::data::timeseries::TimeSeriesCollection;
use crate::models::{CVACore, TimeSeriesSlice, find_matching_ohlcv};

#[allow(unused_imports)]
use crate::config::DEBUG_FLAGS;

// --- The cache key struct ---
#[derive(Clone, Debug)]
struct CacheKey {
    pair: String,
    zone_count: usize,
    time_decay_factor: f64,
    /// Vector of discontinuous slice ranges [(start_idx, end_idx), ...]
    slice_ranges: Vec<(usize, usize)>,
    #[allow(dead_code)]
    price_range: (u64, u64), // Store as bits for Eq/Hash
}

impl Hash for CacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pair.hash(state);
        self.zone_count.hash(state);
        self.time_decay_factor.to_bits().hash(state);
        // Hash each range in the vector
        for range in &self.slice_ranges {
            range.hash(state);
        }
    }
}

#[cfg(debug_assertions)]
fn log_cache_miss_reason(cache: &HashMap<CacheKey, Arc<CVACore>>, requested: &CacheKey) {
    if !DEBUG_FLAGS.print_cva_cache_events {
        return;
    }
    use std::cmp::min;

    let mut same_pair_zone_decay: Option<CacheKey> = None;
    let mut same_pair_zone: Option<CacheKey> = None;
    let mut same_pair: Option<CacheKey> = None;

    for existing in cache.keys() {
        if existing.pair != requested.pair {
            continue;
        }

        if existing.zone_count == requested.zone_count {
            if existing.time_decay_factor.to_bits() == requested.time_decay_factor.to_bits() {
                same_pair_zone_decay = Some(existing.clone());
                break;
            } else if same_pair_zone.is_none() {
                same_pair_zone = Some(existing.clone());
            }
        } else if same_pair.is_none() {
            same_pair = Some(existing.clone());
        }
    }

    if let Some(existing) = same_pair_zone_decay {
        if existing.slice_ranges == requested.slice_ranges {
            log::info!("   ↪ reason: identical key should have hit (possible hash collision)");
            return;
        }

        let len_old = existing.slice_ranges.len();
        let len_new = requested.slice_ranges.len();
        let first_diff_idx = existing
            .slice_ranges
            .iter()
            .zip(requested.slice_ranges.iter())
            .position(|(a, b)| a != b);

        let diff_summary = if let Some(idx) = first_diff_idx {
            format!(
                "first differing range [{}]: old {:?} vs new {:?}",
                idx, existing.slice_ranges[idx], requested.slice_ranges[idx]
            )
        } else if len_old != len_new {
            let preview_idx = min(len_old, len_new);
            let preview_old = existing.slice_ranges.get(preview_idx).cloned();
            let preview_new = requested.slice_ranges.get(preview_idx).cloned();
            format!(
                "range count changed (old {} vs new {}), next old {:?}, new {:?}",
                len_old, len_new, preview_old, preview_new
            )
        } else {
            "slice ranges differ (unable to locate first mismatch)".to_string()
        };

        log::info!(
            "   ↪ reason: slice ranges changed (old {}, new {}) — {}",
            len_old,
            len_new,
            diff_summary
        );
    } else if let Some(existing) = same_pair_zone {
        log::info!(
            "   ↪ reason: time_decay_factor changed {:.6} → {:.6}",
            existing.time_decay_factor,
            requested.time_decay_factor
        );
    } else if let Some(existing) = same_pair {
        log::info!(
            "   ↪ reason: zone_count changed {} → {}",
            existing.zone_count,
            requested.zone_count
        );
    } else if cache.is_empty() {
        log::info!("   ↪ reason: cache is empty (first analysis run)");
    } else {
        log::info!(
            "   ↪ reason: no prior cached entries for pair {} (likely first run in this session)",
            requested.pair
        );
    }
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.pair == other.pair
            && self.zone_count == other.zone_count
            && self.time_decay_factor.to_bits() == other.time_decay_factor.to_bits()
            && self.slice_ranges == other.slice_ranges
    }
}

impl Eq for CacheKey {}

pub struct ZoneGenerator {
    cache: Arc<Mutex<HashMap<CacheKey, Arc<CVACore>>>>,
}

impl Default for ZoneGenerator {
    fn default() -> Self {
        Self {
            // Instantiate an Arc<Mutex> containing an empty HashMap
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Clone for ZoneGenerator {
    fn clone(&self) -> Self {
        Self {
            // Clone the Arc, not the HashMap - this shares the cache!
            cache: Arc::clone(&self.cache),
        }
    }
}

impl ZoneGenerator {
    // Clears the entire cache e.g if source data was changed whilst the app was running (doesn't happen rn)

    pub fn get_cva_results(
        &self,
        selected_pair: &str,
        zone_count: usize,
        time_decay_factor: f64,
        timeseries_data: &TimeSeriesCollection,
        slice_ranges: Vec<(usize, usize)>,
        price_range: (f64, f64),
    ) -> Result<Arc<CVACore>> {
        #[cfg(debug_assertions)]
        let method_start_time = if DEBUG_FLAGS.print_cva_cache_events {
            Some(std::time::Instant::now())
        } else {
            None
        };

        // Return Ref with explicit lifetime
        let key_for_lookup = CacheKey {
            // Use a name that implies it's for lookup/borrowing
            pair: selected_pair.to_string(),
            zone_count,
            time_decay_factor,
            slice_ranges: slice_ranges.clone(),
            price_range: (price_range.0.to_bits(), price_range.1.to_bits()),
        };

        // --- Step 1: Try to get a lock and check if the key exists ---
        {
            if let Ok(cache) = self.cache.lock()
                && let Some(cached_result) = cache.get(&key_for_lookup)
            {
                #[cfg(debug_assertions)]
                if DEBUG_FLAGS.print_cva_cache_events {
                    let total_candles: usize = slice_ranges.iter().map(|(s, e)| e - s).sum();
                    if let Some(start) = method_start_time.as_ref() {
                        log::info!(
                            "CVA Results Cache HIT for {} with {} ranges ({} total candles) and {} zones. Time: {:?}",
                            selected_pair,
                            slice_ranges.len(),
                            total_candles,
                            zone_count,
                            start.elapsed()
                        );
                    }
                }
                return Ok(Arc::clone(cached_result));
            }
        } // Lock is released here.

        // --- Step 2: If not found, compute the results and insert with a lock ---
        let key_for_insertion = key_for_lookup.clone();

        #[cfg(debug_assertions)]
        let computation_start_time = if DEBUG_FLAGS.print_cva_cache_events {
            Some(std::time::Instant::now())
        } else {
            None
        }; // Time just the computation
        #[cfg(debug_assertions)]
        if DEBUG_FLAGS.print_cva_cache_events {
            let total_candles: usize = slice_ranges.iter().map(|(s, e)| e - s).sum();
            log::info!(
                "Cache MISS - Calculating CVA results for {} with {} ranges ({} total candles) and {} zones...",
                selected_pair,
                slice_ranges.len(),
                total_candles,
                zone_count
            );

            if let Ok(cache_guard) = self.cache.lock() {
                log_cache_miss_reason(&cache_guard, &key_for_lookup);
            }
        }

        let computed_results = pair_analysis(
            selected_pair.to_string(),
            zone_count,
            time_decay_factor,
            timeseries_data,
            slice_ranges,
            price_range,
        )?;

        #[cfg(debug_assertions)]
        if DEBUG_FLAGS.print_cva_cache_events {
            if let Some(start) = computation_start_time.as_ref() {
                log::info!(
                    "Computation for {} took: {:?}",
                    selected_pair,
                    start.elapsed()
                );
            }
        }

        let arc_results = Arc::new(computed_results);

        // Insert into cache with lock
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key_for_insertion, Arc::clone(&arc_results));
        }

        // --- Step 3: Return the Arc ---
        Ok(arc_results)
    }
}

fn pair_analysis(
    selected_pair: String,
    zone_count: usize,
    time_decay_factor: f64,
    timeseries_data: &TimeSeriesCollection,
    slice_ranges: Vec<(usize, usize)>,
    price_range: (f64, f64),
) -> Result<CVACore> {
    let ohlcv_time_series = find_matching_ohlcv(
        &timeseries_data.series_data,
        &selected_pair,
        ANALYSIS.interval_width_ms,
    )
    .unwrap_or_else(|_| {
        panic!(
            "No OHLCV data found for pair '{}' with interval {} ms",
            selected_pair, ANALYSIS.interval_width_ms,
        )
    });

    // Validate minimum candle count
    let total_candle_count: usize = slice_ranges.iter().map(|(start, end)| end - start).sum();
    if total_candle_count < ANALYSIS.cva.min_candles_for_analysis {
        bail!(
            "Insufficient data: {} has only {} candles across {} ranges (minimum: {}). \
             This pair is not currently analyzable.",
            selected_pair,
            total_candle_count,
            slice_ranges.len(),
            ANALYSIS.cva.min_candles_for_analysis
        );
    }

    // --- DYNAMIC DECAY LOGIC (Fixed) ---
    let start_idx = slice_ranges.first().map(|r| r.0).unwrap_or(0);
    let end_idx = slice_ranges.last().map(|r| r.1).unwrap_or(0);

    let duration_years = if end_idx > start_idx {
        // Calculate duration directly from indices and interval width
        // (end_idx is exclusive, so end - start = count of intervals covered)
        let duration_ms = (end_idx - start_idx) as f64 * ANALYSIS.interval_width_ms as f64;
        let millis_per_year = 31_536_000_000.0; // 365 * 24 * 3600 * 1000
        duration_ms / millis_per_year
    } else {
        0.0
    };

    let dynamic_decay_factor = if duration_years > 0.0 {
        time_decay_factor.powf(duration_years).max(1.0)
    } else {
        1.0
    };
    // ---------------------------

    let timeseries_slice = TimeSeriesSlice {
        series_data: ohlcv_time_series,
        ranges: slice_ranges.clone(),
    };

    let mut cva_results = timeseries_slice.generate_cva_results(
        zone_count,
        selected_pair.clone(),
        dynamic_decay_factor,
        price_range,
    );

    // Compute start/end timestamps based on the earliest and latest candles
    let first_kline_timestamp = ohlcv_time_series.first_kline_timestamp_ms;

    if let (Some((first_start, _)), Some((_, last_end))) =
        (slice_ranges.first(), slice_ranges.last())
    {
        cva_results.start_timestamp_ms =
            first_kline_timestamp + (*first_start as i64 * ANALYSIS.interval_width_ms);
        cva_results.end_timestamp_ms =
            first_kline_timestamp + (*last_end as i64 * ANALYSIS.interval_width_ms);
    }

    Ok(cva_results)
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use std::sync::Arc;
}
