# Technical Section
## Rust Specification
  Use rust 2024 not rust 2021
  Collapse if statements wherever possible
## Limit lines of code in each rust file.
  1000 line limit.
## Testing
  Do not add ANY #[cfg(test)] tests yet. We will add them later.
## Debug-only code
  Gate all debug-only code using #[cfg(debug_assertions)] 
## Logging code
  Gate logging code with debug flag in `src/config/debug.rs`
## Project Folder Organization
  The mod.rs within each directory is just for storing public modules and re-exports.
  Actual code should go in dedicated file(s) in each module's directory.
  
## Rust crate: eframe -> egui
  1. Important note: we use eframe, not egui directly. So if you want to access an egui component, you need to use the eframe::egui namespace.
  2. We use eframe/egui 0.33. The API is not particularly stable. So please consult latest API documentation before suggesting API calls. Latest API:
    https://docs.rs/egui/0.33.0/egui/ 
### egui API Notes:
  1. Please give each egui UI element a unique ID:
    https://docs.rs/egui/latest/egui/struct.UiBuilder.html#method.id_salt

##  Rust create: egui_plot
  1. We use egui_plot 0.34.0. The API is not particularly stable. So please consult latest API documentation before suggesting API calls. Latest API:
    https://docs.rs/crate/egui_plot/latest
