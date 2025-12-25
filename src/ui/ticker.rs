// use std::collections::HashMap;
use eframe::egui::{Ui, Rect, Pos2, Color32, Sense, Vec2, FontId, OpenUrl};

use crate::config::TICKER;
use crate::engine::SniperEngine;
use crate::utils::TimeUtils;
use crate::ui::utils::format_price;

pub struct TickerItem {
    pub symbol: String,
    pub price: f64,
    pub change: f64, // Difference since last update
    pub last_update_time: f64, // For fade effects
    pub url: Option<String>,
}

pub struct TickerState {
    // Horizontal offset (pixels)
    offset: f32,
    // Local cache to calculate diffs (Symbol -> LastPrice)
    // price_cache: HashMap<String, f64>,
    // The items to render
    items: Vec<TickerItem>,
    // Interactive state
    is_hovered: bool,
    is_dragging: bool,
}

impl Default for TickerState {
    fn default() -> Self {
        Self {
            offset: 0.0,
            // price_cache: HashMap::new(),
            items: Vec::new(),
            is_hovered: false,
            is_dragging: false,
        }
    }
}

impl TickerState {

    pub fn update_data(&mut self, engine: &SniperEngine) {

        // In WASM, we don't update from engine, we use static demo text
        if cfg!(target_arch = "wasm32") {
            if self.items.is_empty() {
                 self.items.push(TickerItem { 
                    symbol: "ZONE SNIPER WEB DEMO".to_string(), price: 0.0, change: 0.0, last_update_time: 0.0,
                    url: None 
                });
                self.items.push(TickerItem { 
                    symbol: "VISIT US ON GITHUB".to_string(), price: 0.0, change: 0.0, last_update_time: 0.0,
                    url: Some("https://github.com/leemthai/sniper".to_string()) 
                });
                self.items.push(TickerItem { 
                    symbol: "GET PRO VERSION FOR LIVE DATA, UNLIMITED TRADING PAIRS AND MUCH MORE".to_string(), price: 0.0, change: 0.0, last_update_time: 0.0 , url: None
                });
                // Add fake price data for demo pairs - TEMP do we really want this?
                self.items.push(TickerItem { 
                    symbol: "BTCUSDT".to_string(), price: 98000.0, change: 120.5, last_update_time: 0.0, url: None 
                });
            }
            return;
        }

        if cfg!(not(target_arch = "wasm32")) {
            let now_ms = crate::utils::TimeUtils::now_timestamp_ms();
            let day_ago_ms = now_ms - (24 * 60 * 60 * 1000); // 24 Hours ago

            // 1. Sync Pairs with Engine
            let pairs = engine.get_all_pair_names();
            
            for pair in pairs {
                if let Some(current_price) = engine.get_price(&pair) {
                    
                    // A. Calculate 24h Change
                    let mut change_24h = 0.0;
                    
                    // Look up history
                    if let Ok(ohlcv) = crate::models::timeseries::find_matching_ohlcv(
                        &engine.timeseries.series_data,
                        &pair,
                        crate::config::ANALYSIS.interval_width_ms
                    ) {
                        // Find index closest to 24h ago
                        let idx_result = ohlcv.timestamps.binary_search(&day_ago_ms);
                        let idx = match idx_result {
                            Ok(i) => i,                 // Exact match
                            Err(i) => i.saturating_sub(1), // Closest previous candle
                        };

                        if idx < ohlcv.close_prices.len() {
                            let old_price = ohlcv.close_prices[idx];
                            if old_price > f64::EPSILON {
                                change_24h = current_price - old_price;
                            }
                        }
                    }

                    // B. Update or Add to List
                    if let Some(item) = self.items.iter_mut().find(|i| i.symbol == pair) {
                        item.price = current_price;
                        item.change = change_24h;
                        // Note: last_update_time is used for 'flash' effects, 
                        // we can update it only if price changed, or just leave it.
                    } else {
                        self.items.push(TickerItem {
                            symbol: pair,
                            price: current_price,
                            change: change_24h,
                            last_update_time: 0.0,
                            url: None,
                        });
                    }
                }
            }

            // 2. Inject Custom Messages
            for (text, url) in TICKER.custom_messages {
                let symbol_key = text.to_string();
                
                // Only add if not already present
                if !self.items.iter().any(|i| i.symbol == symbol_key) {
                    self.items.push(TickerItem {
                        symbol: symbol_key,
                        price: 0.0, // 0.0 marks it as a message/link
                        change: 0.0,
                        last_update_time: 0.0,
                        url: url.map(|s| s.to_string()),
                    });
                }
            }
        }
            
    }

    fn format_item(&self, item: &TickerItem) -> String {
        // 1. Custom Message / Link
        if item.url.is_some() {
             return format!("{} 🔗", item.symbol);
        }
        
        // 2. Static Message (WASM or Custom)
        if item.price == 0.0 && item.change == 0.0 {
            return item.symbol.clone();
        }

        // 3. Formatted Price Pair
        // TEMP we should probably implement some '24hr change' code to show updates.
        let price_str = format_price(item.price);
        let old_price = item.price - item.change;
        let display_threshold = 1e-8;

        let pct = if old_price.abs() > display_threshold {
            (item.change / old_price) * 100.0
        } else {
            0.0
        };

        // 5. Format Change string (Trimmed)
        if item.change.abs() < display_threshold {
             format!("{} {} (0.00%)", item.symbol, price_str)
        } else {
            let sign = if item.change > 0.0 { "+" } else { "" };
            let abs_change = item.change.abs();

            let change_val_str = if abs_change >= 1.0 {
                // Standard prices: 2 decimals fixed
                format!("{:.2}", item.change)
            } else {
                // Small prices: High precision, then TRIM trailing zeros
                // Format to 8 places first
                let s = format!("{:.8}", item.change);
                // Trim '0', then trim '.' if it becomes "0."
                let trimmed = s.trim_end_matches('0').trim_end_matches('.');
                trimmed.to_string()
            };
            
            // Standardize Percent to 2dp
            format!("{} {} ({}{} / {}{:.2}%)", 
                item.symbol, 
                price_str, 
                sign, change_val_str, 
                sign, pct
            )
        }
    }


    pub fn render(&mut self, ui: &mut Ui) -> Option<String> {
        let rect = ui.available_rect_before_wrap();
        let height = TICKER.height;
        let panel_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), height));
        let response = ui.allocate_rect(panel_rect, Sense::click_and_drag());
        ui.painter().rect_filled(panel_rect, 0.0, TICKER.background_color); // Background

        // Interaction Logic
        self.is_hovered = response.hovered();
        self.is_dragging = response.dragged();

        if self.is_dragging {
            // Drag to scrub
            self.offset += response.drag_delta().x;
        } else if !self.is_hovered {
            // FIX: Clamp dt to prevent "Teleporting" during lag spikes.
            // If the frame takes > 50ms, we just accept the slowdown rather than jumping.
            let dt = ui.input(|i| i.stable_dt).min(0.05);
            self.offset -= TICKER.speed_pixels_per_sec * dt;
        }

        // Clip Content (Don't draw outside panel)
        let painter = ui.painter().with_clip_rect(panel_rect);
        let font_id = FontId::monospace(TICKER.font_size);

        // Layout Calculation
        let mut total_width = 0.0;
        let mut clicked_pair = None;

        // Pass 1: Calculate Total Width (for wrapping)
        // We need this to know when to loop.
        for item in &self.items {
            let text = self.format_item(item);
            let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
            total_width += galley.size().x + TICKER.item_spacing;
        }
        
        if total_width < 1.0 { return None; } // No data

        // Wrap offset logic (Infinite Scroll)
        // If offset is too far left (negative), wrap it back to 0
        // If offset is too far right (positive), wrap it back
        self.offset = self.offset % total_width;
        if self.offset > 0.0 { self.offset -= total_width; } // Keep it negative-flowing
        
        // Pass 2: Draw Visible Items
        // We draw items starting at 'self.offset'. 
        // If we reach the end of the list and still have screen space, we loop from start.
        
        let screen_width = panel_rect.width();
        let start_pos = panel_rect.min;
        let loops_needed = (screen_width / total_width).ceil() as i32 + 2;

        for loop_idx in 0..loops_needed {
            let mut loop_x = self.offset + (loop_idx as f32 * total_width);

            for item in &self.items {

                // COLOR LOGIC
                let text_color = if let Some(_) = &item.url {
                    TICKER.text_color_link
                } else if item.price == 0.0 {
                    // Custom Message
                    if TICKER.rainbow_mode {
                        self.get_rainbow_color(loop_x)
                    } else {
                        Color32::GOLD
                    }
                } else {
                    // RESTORED: Green/Red for Pairs based on 24h Change
                    if item.change > f64::EPSILON {
                        TICKER.text_color_up
                    } else if item.change < -f64::EPSILON {
                        TICKER.text_color_down
                    } else {
                        TICKER.text_color_neutral
                    }
                };

                let text_str = self.format_item(item);
                let galley = painter.layout_no_wrap(text_str, font_id.clone(), text_color);
                let w = galley.size().x;
                let h = galley.size().y;
                
                // Draw if visible
                if loop_x + w > 0.0 && loop_x < screen_width {
                    let x_snapped = (start_pos.x + loop_x).round();
                    let y_snapped = (start_pos.y + (height - h) / 2.0).round();
                    let pos = Pos2::new(x_snapped, y_snapped);
                    
                    painter.galley(pos, galley, text_color);

                    // Draw Underline for Links
                    if item.url.is_some() {
                        let line_y = y_snapped + h + 2.0; // 2px gap
                        painter.line_segment(
                            [Pos2::new(x_snapped, line_y), Pos2::new(x_snapped + w, line_y)],
                            (1.0, text_color) // 1px width
                        );
                    }

                    // Click Detection
                    if response.clicked() {
                        if let Some(pointer) = response.interact_pointer_pos() {
                            let item_rect = Rect::from_min_size(pos, Vec2::new(w, height));
                            if item_rect.contains(pointer) {
                                // Handle URL or Pair Click
                                if let Some(url) = &item.url {
                                    ui.ctx().open_url(OpenUrl::new_tab(url));
                                } else if item.price != 0.0 {
                                    clicked_pair = Some(item.symbol.clone());
                                }
                            }
                        }
                    }
                }

                loop_x += w + TICKER.item_spacing;
            }
        }
        
        // Keep animating if we are scrolling
        if !self.is_hovered && !self.is_dragging {
            ui.ctx().request_repaint();
        }

        clicked_pair
    }

    fn get_rainbow_color(&self, x_pos: f32) -> Color32 {
        // Phase based on Time + Position
        let time = TimeUtils::now_timestamp_ms() as f64 / 1000.0;
        let phase = (x_pos as f64 * 0.005) + (time * TICKER.rainbow_speed);
        
        // Simple HSV -> RGB logic or sine waves
        let r = ((phase.sin() * 127.0) + 128.0) as u8;
        let g = (((phase + 2.0).sin() * 127.0) + 128.0) as u8;
        let b = (((phase + 4.0).sin() * 127.0) + 128.0) as u8;
        
        Color32::from_rgb(r, g, b)
    }
}