# NautilusTrader MQL5 Bridge - Build Guide

## Prerequisites

1. **Rust 1.97.1+** (stable) - Install from https://rustup.rs
2. **Visual Studio Build Tools 2022** with "Desktop development with C++" workload
   - Or Visual Studio 2022 Community/Professional with the same workload
   - Download: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022
3. **MetaTrader 5** terminal (for deploying the EA)

## Build Steps

### 1. Install Rust
```powershell
# Download and run rustup-init.exe
# Or use winget:
winget install Rustlang.Rustup
rustup default stable
```

### 2. Install Visual Studio Build Tools
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
# Or manually install from visualstudio.microsoft.com
```

### 3. Build the DLL
```powershell
cd E:\Desktop\neutelus\nautilus_trader
cargo build --release -p nautilus-mql5-bridge
```

The output DLL will be at:
```
target\release\nautilus_mql5_bridge.dll
```

### 4. Deploy to MetaTrader 5
```powershell
# Copy the DLL to MT5's Libraries folder
copy target\release\nautilus_mql5_bridge.dll "$env:APPDATA\MetaQuotes\Terminal\Common\Libraries\"

# Copy the EA to MT5's Experts folder
copy crates\mql5-bridge\mql5\NautilusEA.mq5 "$env:APPDATA\MetaQuotes\Terminal\Common\MQL5\Experts\"
```

## Configuration

The EA accepts a JSON configuration string. Leave empty for defaults:

```json
{
  "strategy": "ema_cross",
  "symbols": ["EURUSD"],
  "fast_period": 12,
  "slow_period": 26,
  "oversold": 30.0,
  "overbought": 70.0
}
```

### Available Strategies
- `ema_cross`: EMA crossover (fast/slow periods)
- `rsi`: RSI mean reversion (uses fast_period as RSI period)

## Quick Test (Backtest Mode)

The EA can run in bar mode (no DLL needed for tick data) using `InpUseBarMode=true`.
This allows backtesting in MT5 Strategy Tester without a live market data feed.

## Troubleshooting

- **"Cannot find link.exe"**: Install Visual Studio Build Tools with C++ workload
- **DLL not loading in MT5**: Enable DLL imports in MT5: Tools > Options > Expert Advisors > Allow DLL imports
- **"Invalid handle" error**: Make sure nt_init() was called successfully in OnInit()
