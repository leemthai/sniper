use {
    chrono::{DateTime, Utc},
    std::time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
pub type AppInstant = std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub type AppInstant = web_time::Instant;

pub struct TimeUtils;

impl TimeUtils {
    // Time constants
    pub const MS_IN_S: i64 = 1000;
    pub const MS_IN_MIN: i64 = Self::MS_IN_S * 60;
    pub const MS_IN_3_MIN: i64 = Self::MS_IN_MIN * 3;
    pub const MS_IN_5_MIN: i64 = Self::MS_IN_MIN * 5;
    pub const MS_IN_15_MIN: i64 = Self::MS_IN_MIN * 15;
    pub const MS_IN_30_MIN: i64 = Self::MS_IN_MIN * 30;
    pub const MS_IN_H: i64 = Self::MS_IN_MIN * 60;
    pub const MS_IN_2_H: i64 = Self::MS_IN_H * 2;
    pub const MS_IN_4_H: i64 = Self::MS_IN_H * 4;
    pub const MS_IN_6_H: i64 = Self::MS_IN_H * 6;
    pub const MS_IN_8_H: i64 = Self::MS_IN_H * 8;
    pub const MS_IN_12_H: i64 = Self::MS_IN_H * 12;
    pub const MS_IN_D: i64 = Self::MS_IN_H * 24;
    pub const MS_IN_3_D: i64 = Self::MS_IN_D * 3;
    pub const MS_IN_W: i64 = Self::MS_IN_D * 7;
    pub const MS_IN_1_M: i64 = Self::MS_IN_D * 30;
    pub const MS_IN_YEAR: i64 = Self::MS_IN_D * 365;

    pub fn interval_to_string(interval_ms: i64) -> &'static str {
        match interval_ms {
            Self::MS_IN_S => "1s",
            Self::MS_IN_MIN => "1m",
            Self::MS_IN_3_MIN => "3m",
            Self::MS_IN_5_MIN => "5m",
            Self::MS_IN_15_MIN => "15m",
            Self::MS_IN_30_MIN => "30m",
            Self::MS_IN_H => "1h",
            Self::MS_IN_2_H => "2h",
            Self::MS_IN_4_H => "4h",
            Self::MS_IN_6_H => "6h",
            Self::MS_IN_8_H => "8h",
            Self::MS_IN_12_H => "12h",
            Self::MS_IN_D => "1d",
            Self::MS_IN_3_D => "3d",
            Self::MS_IN_W => "1w",
            Self::MS_IN_1_M => "1M",
            Self::MS_IN_YEAR => "1Y",
            _ => "unknown",
        }
    }

    /// Returns current UTC time in milliseconds.
    /// Unlike Instant::now(), this *is* WASM safe
    /// On Native, it asks the OS System Clock.
    /// On WASM, the chrono crate bindings internally call JavaScript's Date.now()
    pub fn now_timestamp_ms() -> i64 {
        Utc::now().timestamp_millis()
    }

    pub fn now_utc() -> DateTime<Utc> {
        Utc::now()
    }

    /// Converts a Duration into a specific number of candles based on the interval.
    pub(crate) fn duration_to_candles(duration: Duration, interval_ms: i64) -> usize {
        if interval_ms <= 0 {
            return 0;
        }
        (duration.as_millis() as i64 / interval_ms) as usize
    }

    /// Format as "YYYY-MM-DD"
    pub fn epoch_ms_to_date_string(epoch_ms: i64) -> String {
        let secs = epoch_ms / 1000;
        let dt = DateTime::from_timestamp(secs, 0).unwrap_or_default();
        format!("{}", dt.format("%Y-%m-%d"))
    }

    pub fn format_duration(ms: i64) -> String {
        let secs = ms / 1000;
        if secs < 60 {
            return format!("{}s", secs);
        }
        let mins = secs / 60;
        if mins < 60 {
            return format!("{}m", mins);
        }
        let hours = mins / 60;
        if hours < 24 {
            return format!("{}h", hours);
        }
        let days = hours / 24;
        if days < 30 {
            return format!("{}d", days);
        }
        let months = days / 30;
        if months < 12 {
            return format!("{}M", months);
        }
        let years = months / 12;
        let rem_months = months % 12;
        format!("{}Y {}M", years, rem_months)
    }
}
