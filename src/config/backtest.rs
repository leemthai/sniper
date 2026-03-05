pub(crate) const BACKTEST_PAIR_COUNT: usize = 10; // # pairs to process (actual pairs processed will be random from all loaded pairs coz HashSet unordered)
pub(crate) const BACKTEST_CANDLE_STRIDE: usize = 10; // # candles we stride across when backtesting
pub(crate) const BACKTEST_HOLDOUT_CANDLES: usize = 26_280; // ~3 months of 5-min candles
pub(crate) const BACKTEST_MIN_TRAINING_CANDLES: usize = 576; // ~48 h of 5-min candles — enough for a meaningful similarity scan
pub(crate) const BACKTEST_SKIP_DB_WRITE: bool = true;

pub(crate) const BACKTEST_MODEL_VERSION: &str = "mark-ii";
pub(crate) const BACKTEST_MODEL_DESC: &str = "Walk-forward backtest run";
