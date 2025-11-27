// Std library crates
use std::collections::HashSet;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::time::SystemTime;
// External crates
use anyhow::{Result, bail};
use binance_sdk::common::models::Interval as binance_interval;
use binance_sdk::config::ConfigurationRestApi;
use binance_sdk::models::RestApiRateLimit;
use binance_sdk::spot::{
    SpotRestApi,
    rest_api::{KlinesIntervalEnum, KlinesItemInner, KlinesParams, RestApi},
};
use binance_sdk::{errors, errors::ConnectorError as connection_error};
use tokio::time::{Duration, sleep};
// Local crates
use crate::data::timeseries::intervals::interval_ms_to_string;
use crate::domain::pair_interval::PairInterval;
use crate::utils::time_utils::*; // Bring everything into scope

use crate::config::DEFAULT_KLINES_LIMIT;
use crate::config::binance::{BinanceApiConfig, KLINE_CALL_WEIGHT, WEIGHT_LIMIT_MINUTE};

// When you need DEBUG_PRINT_INTERVAL, use it conditionally:
#[cfg(debug_assertions)]
use crate::config::binance::debug::DEBUG_PRINT_INTERVAL;

#[derive(Debug)]
pub struct AllValidKlines4Pair {
    // A pair name (e.g. "SOLUSDT"), plus the interval scanned, plus a BNKline list (in any order)
    pub klines: Vec<BNKline>,
    pub pair_interval: PairInterval,
}

impl AllValidKlines4Pair {
    // Associated functions.
    pub fn new(klines: Vec<BNKline>, pair_interval: PairInterval) -> Self {
        AllValidKlines4Pair {
            pair_interval,
            klines,
        }
    }

    pub fn first_timestamp_ms(&self) -> i64 {
        self.klines[0].open_timestamp_ms
    }

    pub fn last_timestamp_ms(&self) -> i64 {
        self.klines[self.klines.len() - 1].open_timestamp_ms
    }
}

#[allow(non_snake_case)]
pub fn convert_binance_interval_ms_to_enum(interval_ms: i64) -> KlinesIntervalEnum {
    match interval_ms {
        MS_IN_S => KlinesIntervalEnum::Interval1s,
        MS_IN_MIN => KlinesIntervalEnum::Interval1m,
        MS_IN_3_MIN => KlinesIntervalEnum::Interval3m,
        MS_IN_5_MIN => KlinesIntervalEnum::Interval5m,
        MS_IN_15_MIN => KlinesIntervalEnum::Interval15m,
        MS_IN_30_MIN => KlinesIntervalEnum::Interval30m,
        MS_IN_H => KlinesIntervalEnum::Interval1h,
        MS_IN_2_H => KlinesIntervalEnum::Interval2h,
        MS_IN_4_H => KlinesIntervalEnum::Interval4h,
        MS_IN_6_H => KlinesIntervalEnum::Interval6h,
        MS_IN_8_H => KlinesIntervalEnum::Interval8h,
        MS_IN_12_H => KlinesIntervalEnum::Interval12h,
        MS_IN_D => KlinesIntervalEnum::Interval1d,
        MS_IN_3_D => KlinesIntervalEnum::Interval3d,
        MS_IN_W => KlinesIntervalEnum::Interval1w,
        MS_IN_1_M => KlinesIntervalEnum::Interval1M,
        _ => panic!("Error: unknown interval"),
    }
}
#[allow(dead_code)]
pub fn convert_binance_interval_enum_to_ms(interval: KlinesIntervalEnum) -> i64 {
    match interval {
        KlinesIntervalEnum::Interval1s => MS_IN_S,
        KlinesIntervalEnum::Interval1m => MS_IN_MIN,
        KlinesIntervalEnum::Interval3m => MS_IN_3_MIN,
        KlinesIntervalEnum::Interval5m => MS_IN_5_MIN,
        KlinesIntervalEnum::Interval15m => MS_IN_15_MIN,
        KlinesIntervalEnum::Interval30m => MS_IN_30_MIN,
        KlinesIntervalEnum::Interval1h => MS_IN_H,
        KlinesIntervalEnum::Interval2h => MS_IN_2_H,
        KlinesIntervalEnum::Interval4h => MS_IN_4_H,
        KlinesIntervalEnum::Interval6h => MS_IN_6_H,
        KlinesIntervalEnum::Interval8h => MS_IN_8_H,
        KlinesIntervalEnum::Interval12h => MS_IN_12_H,
        KlinesIntervalEnum::Interval1d => MS_IN_D,
        KlinesIntervalEnum::Interval3d => MS_IN_3_D,
        KlinesIntervalEnum::Interval1w => MS_IN_W,
        KlinesIntervalEnum::Interval1M => MS_IN_1_M,
    }
}

#[allow(clippy::match_overlapping_arm)]
pub fn convert_binance_interval_ms_to_string(interval_ms: i64) -> String {
    interval_ms_to_string(interval_ms).to_string()
}

#[derive(Debug)]
#[allow(dead_code)]
#[derive(PartialOrd, PartialEq)]
pub struct BNKline {
    pub open_timestamp_ms: i64, // only necessary field. All others are optional
    pub open_price: Option<f64>,
    pub high_price: Option<f64>,
    pub low_price: Option<f64>,
    pub close_price: Option<f64>,
    pub base_asset_volume: Option<f64>,
    // pub close_time: i64,
    pub quote_asset_volume: Option<f64>,
    // pub number_of_trades: i64,
    // pub taker_buy_base_asset_volume: String,
    // pub taker_buy_quote_asset_volume: String,
    // pub unused_field: String,
}

// Custom error type for BNKline for better error messages.
#[derive(Debug)] // Added derive for Debug
pub enum BNKlineError {
    InvalidLength,
    InvalidType(String),      // Changed to hold a String
    ConnectionFailed(String), // Added new variant
}

impl fmt::Display for BNKlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BNKlineError::InvalidLength => write!(f, "Invalid length"),
            BNKlineError::InvalidType(string) => write!(f, "Invalid type: {}", string),
            BNKlineError::ConnectionFailed(msg) => {
                write!(f, "Binance API connection failed: {}.", msg)
            }
        }
    }
}

/*
The function's purpose is to safely and cleanly extract a floating-point number from a potentially heterogeneous enum type. It returns a Some(f64) only if the input was the String variant of the enum and that string could be successfully parsed. In all other cases—the input was a different enum variant or the string was invalid—it returns None.
*/
fn convert_kline_item_inner_enum_string_to_float(kline: Option<KlinesItemInner>) -> Option<f64> {
    kline.and_then(|inner| {
        if let KlinesItemInner::String(s) = inner {
            s.parse::<f64>().ok()
        } else {
            None
        }
    })
}

impl Error for BNKlineError {} // Needed in order to compile

// Implement the conversion using the iterator pattern.
impl TryFrom<Vec<KlinesItemInner>> for BNKline {
    type Error = BNKlineError;

    fn try_from(vec_inner_klines: Vec<KlinesItemInner>) -> Result<Self, Self::Error> {
        debug_assert_eq!(12, vec_inner_klines.len());

        let mut items = vec_inner_klines.into_iter();
        let open_timestamp_ms = match items.next().ok_or(BNKlineError::InvalidLength)? {
            KlinesItemInner::Integer(a) => a,
            _ => return Err(BNKlineError::InvalidType("open_time".to_string())),
        };

        // This kind is deffo kinda shitty re: potential errors
        // e.g what happens if convert_klines_inner_to_float goes wrong ???
        // We should just fill a "None" in somehow. Deffo doesn't do that yet.
        let open_price = convert_kline_item_inner_enum_string_to_float(items.next());
        let high_price = convert_kline_item_inner_enum_string_to_float(items.next());
        let low_price = convert_kline_item_inner_enum_string_to_float(items.next());
        let close_price = convert_kline_item_inner_enum_string_to_float(items.next());
        let volume = convert_kline_item_inner_enum_string_to_float(items.next());
        let _ = items.next(); // TEMP this used to be close_time as we don't use it so skip
        let quote_asset_volume = convert_kline_item_inner_enum_string_to_float(items.next());

        // Return the constructed struct on success.
        Ok(BNKline {
            open_timestamp_ms,
            open_price,
            high_price,
            low_price,
            close_price,
            base_asset_volume: volume,
            quote_asset_volume,
        })
    }
}

fn convert_klines(data: Vec<Vec<KlinesItemInner>>) -> Result<Vec<BNKline>, BNKlineError> {
    data.into_iter().map(Vec::try_into).collect()
}

async fn configure_binance_client() -> Result<RestApi, anyhow::Error> {
    let config = BinanceApiConfig::default();
    let rest_conf = ConfigurationRestApi::builder()
        .timeout(config.timeout_ms)
        .retries(config.retries)
        .backoff(config.backoff_ms)
        .build()?;
    // Create the Spot REST API client
    let rest_client = SpotRestApi::production(rest_conf);
    Ok(rest_client)
}

// Helper: set this alias to whatever concrete response type `rest_client.klines(params).await` returns
// Example guess: binance_sdk::spot::rest_api::RestApiResponse<Vec<KlinesItemInner>>
// If the compiler complains about the alias, replace the right-hand path with the concrete type shown in the error.
// type KlinesApiResponse = binance_sdk::spot::rest_api::RestApiResponse<Vec<KlinesItemInner>>;

async fn handle_rate_limits(
    rate_limits: &Option<Vec<RestApiRateLimit>>,
    _pair_interval: &PairInterval,
    concurrent_kline_call_weight: u32,
    #[cfg(debug_assertions)] loop_count: u32,
    bn_weight_limit_minute: u32,
) -> Result<(), anyhow::Error> {
    #[cfg(not(debug_assertions))]
    let _ = &_pair_interval;

    if let Some(value) = rate_limits {
        for rate_limit in value {
            if rate_limit.interval_num == 1 && rate_limit.interval == binance_interval::Minute {
                let current_weight = rate_limit.count;
                let required_headroom =
                    bn_weight_limit_minute.saturating_sub(concurrent_kline_call_weight);
                #[cfg(debug_assertions)]
                if loop_count.is_multiple_of(DEBUG_PRINT_INTERVAL) {
                    log::info!(
                        "Binance min-weight: {} (headroom: {})",
                        current_weight,
                        required_headroom
                    );
                }
                if current_weight > required_headroom {
                    #[cfg(debug_assertions)]
                    log::info!(
                        "{} Current weight ({}) > required headroom ({}) — sleeping until start of next minute",
                        _pair_interval,
                        current_weight,
                        required_headroom,
                    );

                    // Compute time until start of next minute
                    let time_now = SystemTime::now();
                    let duration_since_epoch = time_now
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .expect("Time went backwards");
                    let secs_into_min = duration_since_epoch.as_secs() % 60;
                    let sleep_duration = if secs_into_min == 0 {
                        Duration::from_secs(60)
                    } else {
                        Duration::from_secs(60 - secs_into_min)
                    };

                    #[cfg(debug_assertions)]
                    log::info!(
                        "{} Sleeping for {:?} to reach start of next minute",
                        _pair_interval,
                        sleep_duration
                    );
                    sleep(sleep_duration).await;
                    #[cfg(debug_assertions)]
                    log::info!("Awake at start of a new minute");
                }
            }
        }
    }
    Ok(())
}

fn process_new_klines(
    new_klines: Vec<Vec<KlinesItemInner>>,
    limit_klines_returned: i32,
    all_klines: &mut Vec<BNKline>,
    pair_interval: &PairInterval,
) -> Result<(Option<i64>, bool), anyhow::Error> {
    // Convert to your BNKline
    let mut bn_klines = convert_klines(new_klines).map_err(|e| {
        anyhow::Error::new(e).context(format!("{} convert_klines failed", pair_interval))
    })?;

    if bn_klines.is_empty() {
        bail!(
            "{}: convert_klines produced zero klines (unexpected).",
            pair_interval
        );
    }

    // Will we finish after this batch?
    let mut read_all_klines = false;
    if bn_klines.len() < limit_klines_returned as usize {
        read_all_klines = true;
    }

    // New end_time is open time of first entry in bn_klines
    let end_time = Some(bn_klines[0].open_timestamp_ms);

    // If we already have existing klines, sanity check that last of bn_klines matches first of all_klines
    if !all_klines.is_empty() {
        let last_bn_klines_open_timestamp_ms = &bn_klines[bn_klines.len() - 1].open_timestamp_ms;
        let first_all_klines_open_timestamp_ms = &all_klines[0].open_timestamp_ms;
        debug_assert_eq!(
            last_bn_klines_open_timestamp_ms,
            first_all_klines_open_timestamp_ms
        );
    }

    // Remove the duplicate final item (Binance inclusive behaviour)
    bn_klines.pop();
    if bn_klines.is_empty() {
        // Rare case: the batch had a single item prior to duplicate removal.
        #[cfg(debug_assertions)]
        log::info!(
            "Rare case where new klines was single item before duplicate removal for {}.",
            pair_interval
        );
        // We return true to indicate "batch caused immediate completion"
        all_klines.splice(0..0, Vec::<BNKline>::new());
        return Ok((end_time, true));
    }

    // Prepend the new klines to all_klines
    all_klines.splice(0..0, bn_klines);

    Ok((end_time, read_all_klines))
}

async fn fetch_binance_klines_with_limits(
    rest_client: &RestApi,
    params: KlinesParams,
    pair_interval: &PairInterval,
) -> Result<(Option<Vec<RestApiRateLimit>>, Vec<Vec<KlinesItemInner>>), anyhow::Error> {
    // Make the call
    let response_result = rest_client.klines(params).await;

    match response_result {
        Ok(r) => {
            // Take the rate_limits (Option<Vec<...>>) from the response, then get the inner data
            let rate_limits = r.rate_limits.clone();
            let data = r.data().await?;
            Ok((rate_limits, data))
        }
        Err(e) => {
            // Preserve your original detailed ConnectorError matching / logging
            if let Some(conn_err) = e.downcast_ref::<errors::ConnectorError>() {
                match conn_err {
                    connection_error::ConnectorClientError(msg) => {
                        log::error!(
                            "{} Client error: Check your request parameters. {}",
                            pair_interval,
                            msg
                        );
                    }
                    connection_error::TooManyRequestsError(msg) => {
                        log::error!(
                            "{} Rate limit exceeded. Please wait and try again. {}",
                            pair_interval,
                            msg
                        );
                    }
                    connection_error::RateLimitBanError(msg) => {
                        log::error!(
                            "{} IP address banned due to excessive rate limits. {}",
                            pair_interval,
                            msg
                        );
                    }
                    errors::ConnectorError::ServerError { msg, status_code } => {
                        log::error!(
                            "{} Server error: {} (status code: {:?})",
                            pair_interval,
                            msg,
                            status_code
                        );
                    }
                    errors::ConnectorError::NetworkError(msg) => {
                        log::error!(
                            "{} Network error: Check your internet connection. {}",
                            pair_interval,
                            msg
                        );
                    }
                    errors::ConnectorError::NotFoundError(msg) => {
                        log::error!("Resource not found. {}", msg);
                    }
                    connection_error::BadRequestError(msg) => {
                        log::error!(
                            "{} Bad request: Verify your input parameters. {}",
                            pair_interval,
                            msg
                        );
                    }
                    other => {
                        log::error!("Unexpected ConnectionError variant: {:?}", other);
                    }
                }
                Err(
                    anyhow::Error::new(BNKlineError::ConnectionFailed(conn_err.to_string()))
                        .context(format!("Binance API call failed for {}", pair_interval)),
                )
            } else {
                log::error!(
                    "An unexpected error occurred for {}: {:#}",
                    pair_interval,
                    e
                );
                Err(
                    anyhow::Error::new(BNKlineError::ConnectionFailed(e.to_string())).context(
                        format!("Unexpected error during API call for {}", pair_interval),
                    ),
                )
            }
        }
    }
}

// Required parameters: PairInterval
pub async fn load_klines(
    pair_interval: PairInterval,
    max_simultaneous_kline_calls: u32,
) -> Result<AllValidKlines4Pair, anyhow::Error> {
    let rest_client = configure_binance_client().await?;

    let limit_klines_returned: i32 = 1000; // This can be anywhere from 1 to 1000. My setting though, should be a config.
    let mut end_time: Option<i64> = None;
    const START_TIME: Option<i64> = None;
    let concurrent_kline_call_weight: u32 = KLINE_CALL_WEIGHT * max_simultaneous_kline_calls;
    let mut all_klines: Vec<BNKline> = Vec::new();
    #[cfg(debug_assertions)]
    let mut loop_count = 0;

    loop {
        let params = KlinesParams::builder(
            pair_interval.bn_name().to_string(),
            convert_binance_interval_ms_to_enum(pair_interval.interval_ms),
        )
        .limit(DEFAULT_KLINES_LIMIT) // If not passed in, 500 is used as `limit`
        .end_time(end_time)
        .start_time(START_TIME)
        .build()?;

        // Fetch rate limits + inner kline data in one helper
        let (rate_limits, new_klines) =
            fetch_binance_klines_with_limits(&rest_client, params, &pair_interval).await?;

        // Handle rate-limits (may await/sleep)
        handle_rate_limits(
            &rate_limits,
            &pair_interval,
            concurrent_kline_call_weight,
            #[cfg(debug_assertions)]
            loop_count,
            WEIGHT_LIMIT_MINUTE,
        )
        .await?;

        // Convert & splice the new klines into all_klines
        let (new_end_time, batch_read_all) = process_new_klines(
            new_klines,
            limit_klines_returned,
            &mut all_klines,
            &pair_interval,
        )?;
        end_time = new_end_time;
        if batch_read_all {
            break;
        }

        #[cfg(debug_assertions)]
        {
            loop_count += 1;
        }
    }

    if has_duplicate_kline_open_time(&all_klines) {
        // return Err(anyhow!("Duplicte issue"));
        bail!(
            "has_duplicate_kline_open_time() failed for {} so bailing load_klines()!",
            pair_interval
        );
    } else {
        let pair_kline = AllValidKlines4Pair::new(all_klines, pair_interval);
        Ok(pair_kline)
    }
}

fn has_duplicate_kline_open_time(klines: &[BNKline]) -> bool {
    // Checks whether kline.open_time is duplicated anywhere in the `klines` slice
    let mut seen_ids = HashSet::new();
    for kline in klines {
        if !seen_ids.insert(kline.open_timestamp_ms) {
            // If `insert` returns `false` the element was already present
            return true;
        }
    }
    false
}
