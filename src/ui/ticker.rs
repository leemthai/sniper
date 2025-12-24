use std::collections::HashMap;
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
    price_cache: HashMap<String, f64>,
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
            price_cache: HashMap::new(),
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
                    symbol: "VISIT US ON GITHUB 🔗".to_string(), price: 0.0, change: 0.0, last_update_time: 0.0,
                    url: Some("https://github.com/leemthai/zone-sniper".to_string()) 
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

        if cfg!(not(target_arch = "wasm32"))
        {
            // Native: Sync with Engine Prices
            // We iterate the engine's price list. If price changed, we update our item.
            let pairs = engine.get_all_pair_names();
            
            // Simple strategy: Rebuild list every frame? No, keep stable.
            // Sync Strategy: Update existing items or add new ones.
            for pair in pairs {
                if let Some(new_price) = engine.get_price(&pair) {
                    let old_price = *self.price_cache.get(&pair).unwrap_or(&new_price);
                    
                    // Only update if price changed or new
                    if (new_price - old_price).abs() > f64::EPSILON || !self.price_cache.contains_key(&pair) {
                        let change = new_price - old_price;
                        self.price_cache.insert(pair.clone(), new_price);
                        
                        // Update Item List
                        if let Some(item) = self.items.iter_mut().find(|i| i.symbol == pair) {
                            item.price = new_price;
                            item.change = change;
                            item.last_update_time = TimeUtils::now_timestamp_ms() as f64 / 1000.0;
                        } else {
                            self.items.push(TickerItem {
                                symbol: pair,
                                price: new_price,
                                change: 0.0,
                                last_update_time: TimeUtils::now_timestamp_ms() as f64 / 1000.0,
                                url: None,
                            });
                        }
                    }
                }
            }

            // 2. Inject Custom Messages (Native Only)
            // We check if they are already added to avoid duplicates.
            // Using a special prefix or checking existence by symbol content.
            for (text, url) in TICKER.custom_messages {
                let symbol_key = text.to_string();
                
                // Only add if not present
                if !self.items.iter().any(|i| i.symbol == symbol_key) {
                    self.items.push(TickerItem {
                        symbol: symbol_key,
                        price: 0.0, // Flag for "Message"
                        change: 0.0,
                        last_update_time: TimeUtils::now_timestamp_ms() as f64 / 1000.0,
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
        
        // 2. Text Only Message (Price is 0.0 AND no change)
        if item.price == 0.0 && item.change == 0.0 {
            return item.symbol.clone();
        }

        // 3. Standard Pair
        let sign = if item.change >= 0.0 { "+" } else { "" };
        let price_str = format_price(item.price);
        format!("{} {} ({}{:.3})", item.symbol, price_str, sign, item.change)
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

                // Color Logic: Links override Rainbow. Messages override Rainbow.
                let text_color = if let Some(_) = &item.url {
                    TICKER.text_color_link 
                } else if item.price == 0.0 {
                    Color32::GOLD 
                } else if TICKER.rainbow_mode {
                    self.get_rainbow_color(loop_x)
                } else {
                    if item.change > 0.0 { TICKER.text_color_up } 
                    else if item.change < 0.0 { TICKER.text_color_down }
                    else { TICKER.text_color_neutral }
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