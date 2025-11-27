// src/utils/app_time.rs

#[cfg(not(target_arch = "wasm32"))]
pub type AppInstant = std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub type AppInstant = web_time::Instant;

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> AppInstant {
    std::time::Instant::now()
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> AppInstant {
    web_time::Instant::now()
}
