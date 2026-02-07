use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct GlobalRateLimiter {
    inner: Arc<Mutex<InnerLimiter>>,
}

struct InnerLimiter {
    used_weight: u32,
    // We track the specific minute we are currently counting for
    // e.g. 28,500,123 minutes since Epoch
    current_minute_idx: u64,
    limit: u32,
}

impl GlobalRateLimiter {
    pub(crate) fn new(limit: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerLimiter {
                used_weight: 0,
                current_minute_idx: Self::get_current_minute_idx(),
                limit,
            })),
        }
    }

    /// Acquires permission to use `cost` weight.
    pub(crate) async fn acquire(&self, cost: u32, _context: &str) {
        loop {
            let (wait_duration, _stats) = {
                let mut guard = self.inner.lock().await;
                let now_idx = Self::get_current_minute_idx();

                // 1. Check for New Minute (Wall Clock)
                if now_idx > guard.current_minute_idx {
                    guard.used_weight = 0;
                    guard.current_minute_idx = now_idx;
                }

                // 2. Check Capacity
                if guard.used_weight + cost <= guard.limit {
                    guard.used_weight += cost;
                    return; // Success
                }

                // 3. Calculate Wait (Until next :00)
                let now_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs();

                let seconds_into_minute = now_secs % 60;
                let wait_secs = 60 - seconds_into_minute;

                // Add a tiny buffer (100ms) to ensure we land IN the next minute
                let wait = Duration::from_secs(wait_secs) + Duration::from_millis(100);

                (wait, (guard.used_weight, guard.limit))
            };

            #[cfg(debug_assertions)]
            log::warn!(
                "🛑 Rate Limit Saturated for [{}]. Used: {}/{}. Waiting {:.1}s (until :00)...",
                _context,
                _stats.0,
                _stats.1,
                wait_duration.as_secs_f64()
            );

            tokio::time::sleep(wait_duration).await;
        }
    }

    fn get_current_minute_idx() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs()
            / 60
    }
}
