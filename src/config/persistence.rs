use crate::utils::TimeUtils;

pub struct KlinePersistenceConfig {
    pub directory: &'static str,
    pub filename_base: &'static str,
    pub version: f64,
}

pub struct AppPersistenceConfig {
    pub state_path: &'static str,
    pub ledger_path: &'static str,
}

pub struct PersistenceConfig {
    pub kline: KlinePersistenceConfig,
    pub app: AppPersistenceConfig,
}

pub const PERSISTENCE: PersistenceConfig = PersistenceConfig {
    kline: KlinePersistenceConfig {
        directory: "kline_data",
        filename_base: "kd",
        version: 4.0,
    },
    app: AppPersistenceConfig {
        state_path: ".states.json",
        ledger_path: ".ledger.bin",
    },
};

pub fn kline_cache_filename(interval_ms: i64) -> String {
    let interval_str = TimeUtils::interval_to_string(interval_ms);
    format!(
        "{}_{}_v{}.bin",
        PERSISTENCE.kline.filename_base, interval_str, PERSISTENCE.kline.version
    )
}

#[macro_export]
macro_rules! kline_data_dir {
    () => {
        "kline_data"
    };
}

#[macro_export]
macro_rules! demo_prices_file {
    () => {
        "demo_prices.json"
    };
}

#[macro_export]
macro_rules! demo_cache_file {
    // Note: update this string manually if interval constant changed
    () => {
        "demo_kd_5m_v4.bin"
    };
}
