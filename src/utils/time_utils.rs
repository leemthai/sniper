use chrono::{DateTime, Local, TimeZone, Utc};

pub struct TimeUtils;

impl TimeUtils {
    pub const MS_IN_S: i64 = 1000;
    pub const MS_IN_MIN: i64 = Self::MS_IN_S * 60;
    pub const MS_IN_3_MIN: i64 = Self::MS_IN_S * 60 * 3;
    pub const MS_IN_5_MIN: i64 = Self::MS_IN_S * 60 * 5;
    pub const MS_IN_15_MIN: i64 = Self::MS_IN_S * 60 * 15;
    pub const MS_IN_30_MIN: i64 = Self::MS_IN_S * 60 * 30;
    pub const MS_IN_H: i64 = Self::MS_IN_MIN * 60;
    pub const MS_IN_2_H: i64 = Self::MS_IN_MIN * 60 * 2;
    pub const MS_IN_4_H: i64 = Self::MS_IN_MIN * 60 * 4;
    pub const MS_IN_6_H: i64 = Self::MS_IN_MIN * 60 * 6;
    pub const MS_IN_8_H: i64 = Self::MS_IN_MIN * 60 * 8;
    pub const MS_IN_12_H: i64 = Self::MS_IN_MIN * 60 * 12;
    pub const MS_IN_D: i64 = Self::MS_IN_H * 24;
    pub const MS_IN_3_D: i64 = Self::MS_IN_H * 24 * 3;
    pub const MS_IN_W: i64 = Self::MS_IN_D * 7;
    pub const MS_IN_1_M: i64 = Self::MS_IN_D * 30;
    pub const STANDARD_TIME_FORMAT: &str = "%Y-%m-%d";
    // const STANDARD_TIME_FORMAT: &str = "%d/%m/%Y";

    /// Convert interval in milliseconds to a Binance-style shorthand (e.g. `30m`, `1h`).
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
            _ => "unknown",
        }
    }
}

#[allow(dead_code)]
pub fn epoch_sec_to_local(epoch_sec: i64) -> String {
    // local time not UTC time. Useful for display purposes
    // Utc.timestamp_opt() safely handles the conversion.
    if let chrono::LocalResult::Single(datetime) = Utc.timestamp_opt(epoch_sec, 0) {
        // Format the DateTime object into the desired string
        datetime.format(TimeUtils::STANDARD_TIME_FORMAT).to_string()
    } else {
        // Handle invalid timestamp values
        String::new()
    }
}
#[allow(dead_code)]
pub fn epoch_ms_to_utc(epoch_ms: i64) -> String {
    // Used for display purposes
    epoch_sec_to_utc(epoch_ms / 1000)
}

pub fn epoch_sec_to_utc(epoch_sec: i64) -> String {
    // Used for display purposes
    let dt = DateTime::from_timestamp(epoch_sec, 0).expect("invalid timestamp");
    let formatted = format!("{}", dt.format(TimeUtils::STANDARD_TIME_FORMAT));
    formatted
}

pub fn local_now_as_timestamp_ms() -> i64 {
    let now_local = Local::now();
    now_local.timestamp_millis()
}

pub fn how_many_seconds_ago(past_timestamp_ms: i64) -> i64 {
    // How many seconds ago was the event described by `past_timestamp_ms` ?
    let now_timestamp_ms = local_now_as_timestamp_ms();
    (now_timestamp_ms - past_timestamp_ms) / 1000
}
