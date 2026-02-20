// Std library crates
use std::collections::HashSet;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;

// External crates
use anyhow::{Result, bail};
use binance_sdk::config::ConfigurationRestApi;
use binance_sdk::models::RestApiRateLimit;
use binance_sdk::spot::{
    SpotRestApi,
    rest_api::{KlinesIntervalEnum, KlinesItemInner, KlinesParams, RestApi},
};
use binance_sdk::{errors, errors::ConnectorError as connection_error};

// Local crates
#[cfg(debug_assertions)]
use crate::config::DF;
use crate::config::{
    BINANCE, BaseVol, BinanceApiConfig, ClosePrice, HighPrice, LowPrice, OpenPrice, QuoteVol,
};

use crate::data::rate_limiter::GlobalRateLimiter;
use crate::domain::Candle;
use crate::domain::PairInterval;
use crate::utils::*;

pub trait IntervalToMs {
    fn to_ms(&self) -> i64;
}

// Implement it for the external type
impl IntervalToMs for KlinesIntervalEnum {
    fn to_ms(&self) -> i64 {
        match self {
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
}

// For "MS -> Enum", a static helper is still best,
//    but we return Result instead of panicking.
pub fn try_interval_from_ms(ms: i64) -> Result<KlinesIntervalEnum, String> {
    match ms {
        MS_IN_S => Ok(KlinesIntervalEnum::Interval1s),
        MS_IN_MIN => Ok(KlinesIntervalEnum::Interval1m),
        MS_IN_3_MIN => Ok(KlinesIntervalEnum::Interval3m),
        MS_IN_5_MIN => Ok(KlinesIntervalEnum::Interval5m),
        MS_IN_15_MIN => Ok(KlinesIntervalEnum::Interval15m),
        MS_IN_30_MIN => Ok(KlinesIntervalEnum::Interval30m),
        MS_IN_H => Ok(KlinesIntervalEnum::Interval1h),
        MS_IN_2_H => Ok(KlinesIntervalEnum::Interval2h),
        MS_IN_4_H => Ok(KlinesIntervalEnum::Interval4h),
        MS_IN_6_H => Ok(KlinesIntervalEnum::Interval6h),
        MS_IN_8_H => Ok(KlinesIntervalEnum::Interval8h),
        MS_IN_12_H => Ok(KlinesIntervalEnum::Interval12h),
        MS_IN_D => Ok(KlinesIntervalEnum::Interval1d),
        MS_IN_3_D => Ok(KlinesIntervalEnum::Interval3d),
        MS_IN_W => Ok(KlinesIntervalEnum::Interval1w),
        MS_IN_1_M => Ok(KlinesIntervalEnum::Interval1M),
        _ => Err(format!("Unsupported interval: {}ms", ms)),
    }
}

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

#[derive(Debug, PartialOrd, PartialEq)]
pub struct BNKline {
    pub open_timestamp_ms: i64, // only necessary field. All others are optional
    pub open_price: Option<OpenPrice>,
    pub high_price: Option<HighPrice>,
    pub low_price: Option<LowPrice>,
    pub close_price: Option<ClosePrice>,
    pub base_asset_volume: Option<BaseVol>,
    pub quote_asset_volume: Option<QuoteVol>,
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
            open_price: open_price.map(OpenPrice::new),
            high_price: high_price.map(HighPrice::new),
            low_price: low_price.map(LowPrice::new),
            close_price: close_price.map(ClosePrice::new),
            base_asset_volume: volume.map(BaseVol::new),
            quote_asset_volume: quote_asset_volume.map(QuoteVol::new),
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
        if DF.log_price_stream_updates {
            log::info!(
                "Rare case where new klines was single item before duplicate removal for {}.",
                pair_interval
            );
        }
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
                        log::warn!(
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
                        log::warn!(
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

// Required parameters: PairInterval, Limiter
pub async fn load_klines(
    pair_interval: PairInterval,
    _max_simultaneous_kline_calls: u32, // Unused now, but kept for signature if needed, or remove.
    start_time: Option<i64>,
    limiter: GlobalRateLimiter, // <--- NEW ARGUMENT
) -> Result<AllValidKlines4Pair, anyhow::Error> {
    let rest_client = configure_binance_client().await?;

    let limit_klines_returned: i32 = 1000;
    let mut end_time: Option<i64> = None;
    let mut all_klines: Vec<BNKline> = Vec::new();

    // We use the configured weight (usually 2 for klines)
    let call_weight = BINANCE.limits.kline_call_weight;

    let pair_name = pair_interval.bn_name().to_string();

    loop {
        // GLOBAL RATE LIMIT CHECK
        // This will sleep automatically if the bucket is empty.
        // It coordinates across ALL threads.
        limiter.acquire(call_weight, &pair_name).await;

        let params = KlinesParams::builder(
            pair_interval.bn_name().to_string(),
            try_interval_from_ms(pair_interval.interval_ms)
                .expect("Invalid Binance interval configuration"),
        )
        .limit(BINANCE.limits.klines_limit)
        .end_time(end_time)
        .start_time(start_time)
        .build()?;

        // FETCH (Ignore headers, we track locally)
        let (_rate_limits, new_klines) =
            fetch_binance_klines_with_limits(&rest_client, params, &pair_interval).await?;

        // PROCESS
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
    }

    if has_duplicate_kline_open_time(&all_klines) {
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

impl From<BNKline> for Candle {
    fn from(bn: BNKline) -> Self {
        Candle::new(
            bn.open_timestamp_ms,
            bn.open_price.unwrap_or_default(),
            bn.high_price.unwrap_or_default(),
            bn.low_price.unwrap_or_default(),
            bn.close_price.unwrap_or_default(),
            bn.base_asset_volume.unwrap_or_default(),
            bn.quote_asset_volume.unwrap_or_default(),
        )
    }
}
