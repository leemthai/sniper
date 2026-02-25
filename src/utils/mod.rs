mod maths_utils;
mod perf;
mod time_utils;

pub use time_utils::{AppInstant, TimeUtils};

pub(crate) use maths_utils::{mean_and_stddev, normalize_max, remap, smooth_data};
