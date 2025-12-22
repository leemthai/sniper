#[macro_export]
macro_rules! trace_time {
    // $threshold_micros: Enter 500 for 0.5ms, 1000 for 1ms, etc.
    ($name:expr, $threshold_micros:expr, $block:block) => {{
        // Check the global flag
        if crate::config::DEBUG_FLAGS.enable_perf_logging {
            // FIX: Use AppInstant instead of std::time::Instant
            let start = crate::utils::time_utils::AppInstant::now();
            let result = $block;
            let elapsed = start.elapsed();
            let micros = elapsed.as_micros();
            
            if micros > $threshold_micros {
                let mode = if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" };
                
                log::error!( 
                    "🐢 SLOW [{}]: '{}' took {:.3}ms (Threshold: {:.3}ms)", 
                    mode,
                    $name, 
                    micros as f64 / 1000.0, 
                    $threshold_micros as f64 / 1000.0
                );
            }
            result
        } else {
            // No-op mode
            $block
        }
    }};
}