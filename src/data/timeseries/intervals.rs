//! Interval helper utilities shared between native and WASM builds.

use crate::utils::time_utils::{
    MS_IN_1_M, MS_IN_2_H, MS_IN_3_D, MS_IN_3_MIN, MS_IN_4_H, MS_IN_5_MIN, MS_IN_6_H, MS_IN_8_H,
    MS_IN_12_H, MS_IN_15_MIN, MS_IN_30_MIN, MS_IN_D, MS_IN_H, MS_IN_MIN, MS_IN_S, MS_IN_W,
};

/// Convert interval in milliseconds to a Binance-style shorthand (e.g. `30m`, `1h`).
pub fn interval_ms_to_string(interval_ms: i64) -> &'static str {
    match interval_ms {
        MS_IN_S => "1s",
        MS_IN_MIN => "1m",
        MS_IN_3_MIN => "3m",
        MS_IN_5_MIN => "5m",
        MS_IN_15_MIN => "15m",
        MS_IN_30_MIN => "30m",
        MS_IN_H => "1h",
        MS_IN_2_H => "2h",
        MS_IN_4_H => "4h",
        MS_IN_6_H => "6h",
        MS_IN_8_H => "8h",
        MS_IN_12_H => "12h",
        MS_IN_D => "1d",
        MS_IN_3_D => "3d",
        MS_IN_W => "1w",
        MS_IN_1_M => "1M",
        _ => "unknown",
    }
}
