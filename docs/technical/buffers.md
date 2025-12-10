Here is the breakdown of exactly what "The Swap" means in Rust terms and where it lives in our code.

### 1. What does "Sending the Arc" mean?

In Rust, an `Arc` (Atomic Reference Counted) is just a **smart pointer**. Think of it like a remote control that points to a specific TV channel (the data in memory).

When we say the Worker "sends the Arc," we mean:
1.  The Worker calculates the heavy data (`TradingModel`) in its own isolated memory.
2.  It wraps that data in an `Arc`.
3.  It puts **the pointer** (not the data) into a mailbox (the `mpsc::channel`).
4.  The Worker forgets about it. The data didn't move; only ownership of the "remote control" moved.

### 2. What is "The Swap"?

"The Swap" happens on the **Main Thread** (inside the Engine update loop).

The Engine holds a struct (`PairState`) which has a field `pub model: Option<Arc<TradingModel>>`.

*   **Before Swap:** `model` holds "Pointer A" (Old Data).
*   **The Swap Action:** The Engine takes "Pointer B" (New Data) from the mailbox and executes:
    ```rust
    self.model = Some(new_model_arc);
    ```
*   **After Swap:** `model` now holds "Pointer B".

**Why is this "Wait-Free" for the UI?**
This is the magic of `Arc`.
1.  When the UI asks for data to draw a frame, it calls `.clone()` on the Arc. This gives the UI **its own copy of the pointer**.
2.  While the UI is drawing Frame 1 using "Pointer A", the Engine might swap the main storage to "Pointer B".
3.  **The UI doesn't care.** It is still holding "Pointer A" validly. It finishes drawing Frame 1 safely.
4.  On Frame 2, the UI asks again, and this time gets "Pointer B".
5.  "Pointer A" is automatically deleted from memory only when *nobody* is looking at it anymore.

No locks. No waiting. The UI *never* has to wait for a calculation to finish; it just draws whatever pointer it grabs at the start of the frame.

### 3. Has this been coded yet?

**Yes.** We wrote the mechanism for this in the previous session.

**A. The Worker Sending (src/engine/worker.rs)**
```rust
// The worker builds the data...
let model = TradingModel::from_cva(...);

// The worker wraps it in Arc::new() and sends it down the channel (tx)
tx.send(JobResult {
    // ...
    result: Ok(Arc::new(model)), // <--- Sending the Arc
    // ...
}).unwrap();
```

**B. The Engine Receiving (src/engine/core.rs)**
```rust
// In update() loop...
while let Ok(result) = self.result_rx.try_recv() {
    self.handle_job_result(result);
}

// In handle_job_result...
match result.result {
    Ok(model) => {
        // ...
        state.update_buffer(model); // <--- Passing the Arc to the State
    }
}
```

**C. The Swap Implementation (src/engine/state.rs)**
```rust
pub fn update_buffer(&mut self, new_model: Arc<TradingModel>) {
    // THIS IS THE SWAP.
    // Overwriting 'self.model' drops the old pointer and sets the new one.
    // It takes nanoseconds.
    self.model = Some(new_model); 
    
    // ... reset flags ...
}
```