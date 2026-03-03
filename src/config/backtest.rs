pub(crate) const BACKTEST_PAIR_COUNT: usize = 1; // # pairs to process (actual pairs processed will be random coz HashSet unordered)
pub(crate) const BACKTEST_CANDLE_STRIDE: usize = 10; // # candles we stride across when backtesting
pub(crate) const BACKTEST_HOLDOUT_CANDLES: usize = 26_280;
pub(crate) const BACKTEST_MIN_TRAINING_CANDLES: usize = 576; // ~48 h of 5-min candles — enough for a meaningful similarity scan
