use {
    crate::{
        app::{BASE_INTERVAL, Price, PriceLike},
        engine::SniperEngine,
        models::find_matching_ohlcv,
        utils::{AppInstant, TimeUtils},
    },
    eframe::egui::{Color32, FontId, OpenUrl, Pos2, Rect, Sense, Ui, Vec2},
};

pub struct TickerConfig {
    pub background_color: Color32,
    pub custom_messages: &'static [(&'static str, Option<&'static str>)],
    pub font_size: f32,
    pub height: f32,
    pub item_spacing: f32,
    pub min_change_pct_for_color: f64,
    pub rainbow_mode: bool,
    pub rainbow_speed: f64,
    pub speed_pixels_per_sec: f32,
    pub text_color_down: Color32,
    pub text_color_link: Color32,
    pub text_color_neutral: Color32,
    pub text_color_up: Color32,
}

pub const TICKER: TickerConfig = TickerConfig {
    background_color: Color32::from_rgb(10, 10, 15),
    custom_messages: &[
        ("🎄 MERRY CHRISTMAS 🎄", None),
        (
            "Built with Rust \u{e7a8}",
            Some("https://www.rust-lang.org"),
        ),
        (
            "Zone Sniper Pro",
            Some("https://github.com/leemthai/sniper"),
        ),
    ],
    font_size: 10.0,
    height: 18.0,
    item_spacing: 40.0,
    min_change_pct_for_color: 0.01,
    rainbow_mode: true,
    rainbow_speed: 2.0,
    speed_pixels_per_sec: 30.0,
    text_color_down: Color32::RED,
    text_color_link: Color32::from_rgb(100, 200, 255),
    text_color_neutral: Color32::LIGHT_GRAY,
    text_color_up: Color32::GREEN,
};

pub(crate) struct TickerItem {
    pub symbol: String,
    pub price: Price,
    pub change: f64,
    pub url: Option<String>,
}

pub(crate) struct TickerState {
    offset: f32,
    items: Vec<TickerItem>,
    is_hovered: bool,
    is_dragging: bool,
    last_render_time: Option<AppInstant>,
}

impl Default for TickerState {
    fn default() -> Self {
        Self {
            offset: 0.0,
            items: Vec::new(),
            is_hovered: false,
            is_dragging: false,
            last_render_time: None,
        }
    }
}

impl TickerState {
    pub(crate) fn update_data(&mut self, engine: &SniperEngine) {
        if cfg!(target_arch = "wasm32") {
            if self.items.is_empty() {
                self.items.push(TickerItem {
                    symbol: "ZONE SNIPER WEB DEMO".to_string(),
                    price: Price::new(0.0),
                    change: 0.0,
                    url: None,
                });
                self.items.push(TickerItem {
                    symbol: "VISIT US ON GITHUB".to_string(),
                    price: Price::new(0.0),
                    change: 0.0,
                    url: Some("https://github.com/leemthai/sniper".to_string()),
                });
                self.items.push(TickerItem {
                    symbol: "GET PRO VERSION FOR LIVE DATA, UNLIMITED TRADING PAIRS AND MUCH MORE"
                        .to_string(),
                    price: Price::new(0.0),
                    change: 0.0,
                    url: None,
                });
                self.items.push(TickerItem {
                    symbol: "BTCUSDT".to_string(),
                    price: Price::new(98000.0),
                    change: 120.5,
                    url: None,
                });
            }
            return;
        }

        if cfg!(not(target_arch = "wasm32")) {
            let now_ms = TimeUtils::now_timestamp_ms();
            let day_ago_ms = now_ms - TimeUtils::MS_IN_D;
            let pairs = engine.get_all_pair_names();
            for pair in pairs {
                if let Some(current_price) = engine.get_price(&pair) {
                    let mut change_24h = 0.0;
                    let ts_guard = engine.timeseries.read().unwrap();
                    if let Ok(ohlcv) = find_matching_ohlcv(
                        &ts_guard.series_data,
                        &pair,
                        BASE_INTERVAL.as_millis() as i64,
                    ) {
                        let idx_result = ohlcv.timestamps.binary_search(&day_ago_ms);
                        let idx = match idx_result {
                            Ok(i) => i,
                            Err(i) => i.saturating_sub(1),
                        };
                        if idx < ohlcv.close_prices.len() {
                            let old_price = ohlcv.close_prices[idx];
                            if old_price.is_positive() {
                                change_24h = current_price - old_price.into();
                            }
                        }
                    }
                    if let Some(item) = self.items.iter_mut().find(|i| i.symbol == pair) {
                        item.price = current_price;
                        item.change = change_24h;
                    } else {
                        self.items.push(TickerItem {
                            symbol: pair,
                            price: current_price,
                            change: change_24h,
                            url: None,
                        });
                    }
                }
            }
            for (text, url) in TICKER.custom_messages {
                let symbol_key = text.to_string();
                if !self.items.iter().any(|i| i.symbol == symbol_key) {
                    self.items.push(TickerItem {
                        symbol: symbol_key,
                        price: Price::new(0.0), // 0.0 marks as message/link
                        change: 0.0,
                        url: url.map(|s| s.to_string()),
                    });
                }
            }
        }
    }

    pub(crate) fn render(&mut self, ui: &mut Ui) -> Option<String> {
        let now = AppInstant::now();
        let dt = if let Some(last) = self.last_render_time {
            let duration = now.duration_since(last).as_secs_f32();
            duration.min(0.10)
        } else {
            0.0
        };
        self.last_render_time = Some(now);

        let rect = ui.available_rect_before_wrap();
        let height = TICKER.height;
        let panel_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), height));
        let response = ui.allocate_rect(panel_rect, Sense::click_and_drag());
        ui.painter()
            .rect_filled(panel_rect, 0.0, TICKER.background_color);
        self.is_hovered = response.hovered();
        self.is_dragging = response.dragged();
        if self.is_dragging {
            self.offset += response.drag_delta().x;
        } else if !self.is_hovered {
            self.offset -= TICKER.speed_pixels_per_sec * dt;
        }
        let painter = ui.painter().with_clip_rect(panel_rect);
        let font_id = FontId::monospace(TICKER.font_size);
        let mut total_width = 0.0;
        let mut clicked_pair = None;
        for item in &self.items {
            let text = self.format_item(item);
            let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
            total_width += galley.size().x + TICKER.item_spacing;
        }

        if total_width < 1.0 {
            return None;
        }
        self.offset %= total_width;
        if self.offset > 0.0 {
            self.offset -= total_width;
        }
        let screen_width = panel_rect.width();
        let start_pos = panel_rect.min;
        let loops_needed = (screen_width / total_width).ceil() as i32 + 2;
        for loop_idx in 0..loops_needed {
            let mut loop_x = self.offset + (loop_idx as f32 * total_width);
            for item in &self.items {
                let text_color = if item.url.is_some() {
                    TICKER.text_color_link
                } else if item.price.value() == 0.0 {
                    if TICKER.rainbow_mode {
                        self.get_rainbow_color(loop_x)
                    } else {
                        Color32::GOLD
                    }
                } else {
                    let pct = self.calc_pct(item);
                    if pct > TICKER.min_change_pct_for_color {
                        TICKER.text_color_up
                    } else if pct < -TICKER.min_change_pct_for_color {
                        TICKER.text_color_down
                    } else {
                        TICKER.text_color_neutral
                    }
                };

                let text_str = self.format_item(item);
                let galley = painter.layout_no_wrap(text_str, font_id.clone(), text_color);
                let w = galley.size().x;
                let h = galley.size().y;
                if loop_x + w > 0.0 && loop_x < screen_width {
                    let x_snapped = (start_pos.x + loop_x).round();
                    let y_snapped = (start_pos.y + (height - h) / 2.0).round();
                    let pos = Pos2::new(x_snapped, y_snapped);
                    painter.galley(pos, galley, text_color);
                    if item.url.is_some() {
                        let line_y = y_snapped + h + 2.0; // 2px gap
                        painter.line_segment(
                            [
                                Pos2::new(x_snapped, line_y),
                                Pos2::new(x_snapped + w, line_y),
                            ],
                            (1.0, text_color), // 1px width
                        );
                    }
                    if response.clicked() {
                        if let Some(pointer) = response.interact_pointer_pos() {
                            let item_rect = Rect::from_min_size(pos, Vec2::new(w, height));
                            if item_rect.contains(pointer) {
                                if let Some(url) = &item.url {
                                    ui.ctx().open_url(OpenUrl::new_tab(url));
                                } else if item.price.value() != 0.0 {
                                    clicked_pair = Some(item.symbol.clone());
                                }
                            }
                        }
                    }
                }
                loop_x += w + TICKER.item_spacing;
            }
        }
        if !self.is_hovered && !self.is_dragging {
            ui.ctx().request_repaint();
        }

        clicked_pair
    }

    fn format_item(&self, item: &TickerItem) -> String {
        if item.url.is_some() {
            return format!("{} 🔗", item.symbol);
        }
        if item.price.value() == 0.0 && item.change == 0.0 {
            return item.symbol.clone();
        }
        let price_str = format!("{}", item.price);
        let pct = self.calc_pct(item);
        let abs_change = item.change.abs();
        let precision = if abs_change < 0.0001 {
            6
        } else if abs_change < 1.0 {
            4
        } else {
            2
        };
        let is_zero = pct.abs() < TICKER.min_change_pct_for_color;
        let sign_change = if is_zero {
            " "
        } else if item.change > 0.0 {
            "+"
        } else {
            "-"
        };
        let sign_pct = if is_zero {
            " "
        } else if pct > 0.0 {
            "+"
        } else {
            "-"
        };

        format!(
            "{} {} ({}{:.prec$} / {}{:.2}%)",
            item.symbol,
            price_str,
            sign_change,
            abs_change,
            sign_pct,
            pct.abs(),
            prec = precision
        )
    }

    fn calc_pct(&self, item: &TickerItem) -> f64 {
        let price_p: Price = item.price;
        let old_price = price_p.value() - item.change;

        if old_price.abs() > f64::EPSILON {
            (item.change / old_price) * 100.0
        } else {
            0.0
        }
    }

    fn get_rainbow_color(&self, x_pos: f32) -> Color32 {
        let time = TimeUtils::now_timestamp_ms() as f64 / 1000.0;
        let phase = (x_pos as f64 * 0.005) + (time * TICKER.rainbow_speed);
        let r = ((phase.sin() * 127.0) + 128.0) as u8;
        let g = (((phase + 2.0).sin() * 127.0) + 128.0) as u8;
        let b = (((phase + 4.0).sin() * 127.0) + 128.0) as u8;

        Color32::from_rgb(r, g, b)
    }
}
