 # Simulation system
 ## The system preserves the simulated price on exit
  i.e. come back into sim mode, the simulated price is restored)
 ##  Keys:
     S - Simulation mode (toggle)
     D - Direction of price movement (toggle)
     X - rotate cycle step size
     A - activate price change
     4 - Jump to next sticky zone (respect `Direction`)
     5 - Jump to next slippy zone (respect `Direction`)
     6 - Jump to next reversal zone (respect `Direction`)

## Sanity test checklist

1. Enter simulation mode with `S` and confirm the status banner switches to **SIMULATION MODE**.
2. Tap `A` ten times at the default 0.1% step and verify exactly one CVA recalculation runs (new trigger system).
3. Toggle direction with `D`, press `A` once, and ensure no additional recalcs occur unless the cumulative move crosses the 1% threshold.
4. Use `4`–`6` hotkeys to jump to zones and confirm the plot updates without forcing extra recalcs every frame.
5. Exit simulation mode with `S` and verify live pricing resumes (status banner returns to **LIVE MODE**).

## Questions:
  what if simulated price is pushed beyond current range?
  hmmm... that's weird. not really should happen.
  If simulated price changes then the range should change too maybe?
