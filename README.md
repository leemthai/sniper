# Zone Sniper
### Scope. Lock. Snipe.

> **A high-performance market structure analysis engine written in Rust.**

Zone Sniper is a trading intelligence tool that identifies high-probability opportunities by fingerprinting the current market state (Volatility, Momentum, Volume) and matching it against historical data using a multi-threaded Pathfinder simulation.

**[Launch Web Demo](https://leemthai.github.io/sniper/)** | **[View Source](https://github.com/leemthai/sniper)**

---

## 🎯 What it does
Most trading tools look at indicators (RSI, MACD). Zone Sniper looks at **Structure** and **Probability**.

1.  **Zone Identification**: Automatically detects "Sticky Zones" (High Volume Nodes) and "Rejection Zones" (Wick Clusters) using Cumulative Volume Analysis (CVA).
2.  **Market Fingerprinting**: Captures the exact "DNA" of the market *right now* (e.g., "High Volatility, Downward Momentum, Low Volume").
3.  **Pathfinder Simulation**: Scans the entire price history of the asset (500k+ candles) to find the top 50 historical moments that match today's DNA.
4.  **Ghost Runner**: Simulates those 50 historical scenarios to calculate the exact Win Rate, Expected Return (ROI), and optimal Stop Loss placement for the current setup.

## 🚀 Web Demo vs. Native App

This project compiles to both native desktop (Windows/Linux/Mac) and WebAssembly (WASM).

| Feature | Web Demo (WASM) | Native App |
| :--- | :--- | :--- |
| **Data Source** | Embedded Static Cache (Offline) | **Live Binance API (WebSocket + REST)** |
| **Asset Pairs** | Limited Set (BTC, ETH, SOL...) | **Unlimited (All Binance Pairs)** |
| **Database** | Memory Only | **SQLite (Persistent Cache)** |
| **Performance** | Single Threaded (Browser Limit) | **Multi-Threaded (Rayon Parallelism)** |
| **Analysis** | Snapshot | **Real-time Streaming** |

**[Try the Web Demo here](https://leemthai.github.io/sniper/)** to see the UI and visualization engine in action.

## 🛠️ Technical Stack
This project serves as a demonstration of high-performance Rust engineering:
*   **Core**: Rust 2024 (Edition).
*   **GUI**: `egui` (Immediate Mode GUI) for 60fps rendering.
*   **Concurrency**: `tokio` for async I/O, `rayon` for parallel data processing.
*   **Math**: Custom statistical engines for CVA and Pathfinder simulations.
*   **Platform**: Cross-compiled to `wasm32-unknown-unknown` for browser deployment.

## ⚖️ License & Usage
**PolyForm Noncommercial License 1.0.0**

This software is **Source Available**.
*   ✅ You may view, compile, and use this software for personal, non-commercial purposes.
*   ✅ You may use this code for educational purposes.
*   ❌ You may **not** sell this software, provide it as a service (SaaS), or use it for commercial gain without a license.

## 🤝 Contact & Services
I built Zone Sniper to demonstrate how complex data analysis can be visualized simply and effectively.

**I am available for custom Rust development, high-performance UI design, and financial software engineering.**

*   **Email**: zonesniper250@gmail.com
*   **GitHub**: [leemthai](https://github.com/leemthai)