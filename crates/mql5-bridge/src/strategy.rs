use std::collections::HashMap;

use crate::types::Signal;

pub trait TradingStrategy: Send {
    fn on_tick(&mut self, symbol: &str, bid: f64, ask: f64, volume: f64) -> Signal;
    fn on_bar(
        &mut self,
        symbol: &str,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        timestamp_ns: i64,
    ) -> Signal;
    fn on_imbalance(&mut self, _symbol: &str, _imbalance: f64) -> Signal {
        Signal::None
    }
    fn on_orderflow(&mut self, _symbol: &str, _delta: f64, _volume: f64) -> Signal {
        Signal::None
    }
    fn signal(&mut self, symbol: &str) -> Signal;
    fn name(&self) -> &str;
}

struct EMACrossState {
    fast_ema: f64,
    slow_ema: f64,
    fast_alpha: f64,
    slow_alpha: f64,
    prev_fast: f64,
    prev_slow: f64,
    initialized: bool,
    current_signal: Signal,
}

impl EMACrossState {
    fn new(fast_period: usize, slow_period: usize) -> Self {
        Self {
            fast_ema: 0.0,
            slow_ema: 0.0,
            fast_alpha: 2.0 / (fast_period as f64 + 1.0),
            slow_alpha: 2.0 / (slow_period as f64 + 1.0),
            prev_fast: 0.0,
            prev_slow: 0.0,
            initialized: false,
            current_signal: Signal::None,
        }
    }

    fn update(&mut self, price: f64) {
        if !self.initialized {
            self.fast_ema = price;
            self.slow_ema = price;
            self.initialized = true;
            return;
        }

        self.prev_fast = self.fast_ema;
        self.prev_slow = self.slow_ema;

        self.fast_ema = price * self.fast_alpha + self.fast_ema * (1.0 - self.fast_alpha);
        self.slow_ema = price * self.slow_alpha + self.slow_ema * (1.0 - self.slow_alpha);

        if self.prev_fast <= self.prev_slow && self.fast_ema > self.slow_ema {
            self.current_signal = Signal::Buy;
        } else if self.prev_fast >= self.prev_slow && self.fast_ema < self.slow_ema {
            self.current_signal = Signal::Sell;
        } else if self.current_signal == Signal::Buy
            && self.fast_ema < self.slow_ema
        {
            self.current_signal = Signal::Exit;
        } else if self.current_signal == Signal::Sell
            && self.fast_ema > self.slow_ema
        {
            self.current_signal = Signal::Exit;
        }
    }

}

pub struct EMACrossStrategy {
    name: String,
    symbols: HashMap<String, EMACrossState>,
}

impl EMACrossStrategy {
    pub fn new(symbols: Vec<String>, fast_period: usize, slow_period: usize) -> Self {
        let mut state = HashMap::new();
        for sym in &symbols {
            state.insert(
                sym.clone(),
                EMACrossState::new(fast_period, slow_period),
            );
        }

        Self {
            name: format!("EMACross_{}_{}", fast_period, slow_period),
            symbols: state,
        }
    }
}

impl TradingStrategy for EMACrossStrategy {
    fn on_tick(&mut self, symbol: &str, bid: f64, ask: f64, _: f64) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            let mid = (bid + ask) / 2.0;
            state.update(mid);
            state.current_signal
        } else {
            Signal::None
        }
    }

    fn on_bar(
        &mut self,
        symbol: &str,
        _open: f64,
        _high: f64,
        _low: f64,
        close: f64,
        _volume: f64,
        _timestamp_ns: i64,
    ) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            state.update(close);
            state.current_signal
        } else {
            Signal::None
        }
    }

    fn signal(&mut self, symbol: &str) -> Signal {
        self.symbols
            .get(symbol)
            .map(|s| s.current_signal)
            .unwrap_or(Signal::None)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct RSIMeanReversionStrategy {
    name: String,
    symbols: HashMap<String, RSIState>,
    oversold: f64,
    overbought: f64,
}

struct RSIState {
    period: usize,
    gains: Vec<f64>,
    losses: Vec<f64>,
    avg_gain: f64,
    avg_loss: f64,
    rsi: f64,
    initialized: bool,
    current_signal: Signal,
    prev_price: Option<f64>,
}

impl RSIState {
    fn new(period: usize) -> Self {
        Self {
            period,
            gains: Vec::with_capacity(period),
            losses: Vec::with_capacity(period),
            avg_gain: 0.0,
            avg_loss: 0.0,
            rsi: 50.0,
            initialized: false,
            current_signal: Signal::None,
            prev_price: None,
        }
    }

    fn update(&mut self, price: f64) {
        if let Some(prev) = self.prev_price {
            let change = price - prev;
            let gain = change.max(0.0);
            let loss = (-change).max(0.0);

            if !self.initialized {
                self.gains.push(gain);
                self.losses.push(loss);

                if self.gains.len() >= self.period {
                    self.avg_gain = self.gains.iter().sum::<f64>() / self.period as f64;
                    self.avg_loss = self.losses.iter().sum::<f64>() / self.period as f64;
                    self.initialized = true;
                }
            } else {
                self.avg_gain =
                    (self.avg_gain * (self.period as f64 - 1.0) + gain) / self.period as f64;
                self.avg_loss =
                    (self.avg_loss * (self.period as f64 - 1.0) + loss) / self.period as f64;

                self.rsi = if self.avg_loss == 0.0 {
                    100.0
                } else {
                    let rs = self.avg_gain / self.avg_loss;
                    100.0 - (100.0 / (1.0 + rs))
                };
            }
        }

        self.prev_price = Some(price);
    }
}

impl RSIMeanReversionStrategy {
    pub fn new(symbols: Vec<String>, period: usize, oversold: f64, overbought: f64) -> Self {
        let mut state = HashMap::new();
        for sym in &symbols {
            state.insert(sym.clone(), RSIState::new(period));
        }

        Self {
            name: format!("RSI_{}_{}_{}", period, oversold as u32, overbought as u32),
            symbols: state,
            oversold,
            overbought,
        }
    }
}

impl TradingStrategy for RSIMeanReversionStrategy {
    fn on_tick(&mut self, symbol: &str, bid: f64, ask: f64, _: f64) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            let mid = (bid + ask) / 2.0;
            state.update(mid);

            if !state.initialized {
                return Signal::None;
            }

            if state.rsi < self.oversold && state.current_signal != Signal::Buy {
                state.current_signal = Signal::Buy;
            } else if state.rsi > self.overbought && state.current_signal != Signal::Sell {
                state.current_signal = Signal::Sell;
            } else if state.rsi > 50.0 && state.current_signal == Signal::Buy {
                state.current_signal = Signal::Exit;
            } else if state.rsi < 50.0 && state.current_signal == Signal::Sell {
                state.current_signal = Signal::Exit;
            }

            state.current_signal
        } else {
            Signal::None
        }
    }

    fn on_bar(
        &mut self,
        symbol: &str,
        _open: f64,
        _high: f64,
        _low: f64,
        close: f64,
        _volume: f64,
        _timestamp_ns: i64,
    ) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            state.update(close);

            if !state.initialized {
                return Signal::None;
            }

            if state.rsi < self.oversold && state.current_signal != Signal::Buy {
                state.current_signal = Signal::Buy;
            } else if state.rsi > self.overbought && state.current_signal != Signal::Sell {
                state.current_signal = Signal::Sell;
            } else if state.rsi > 50.0 && state.current_signal == Signal::Buy {
                state.current_signal = Signal::Exit;
            } else if state.rsi < 50.0 && state.current_signal == Signal::Sell {
                state.current_signal = Signal::Exit;
            }

            state.current_signal
        } else {
            Signal::None
        }
    }

    fn signal(&mut self, symbol: &str) -> Signal {
        self.symbols
            .get(symbol)
            .map(|s| s.current_signal)
            .unwrap_or(Signal::None)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ── Order Book Imbalance Strategy ─────────────────────────────────

pub struct OrderBookImbalanceStrategy {
    name: String,
    symbols: HashMap<String, OrderBookImbalanceState>,
    entry_threshold: f64,
    exit_threshold: f64,
}

struct OrderBookImbalanceState {
    smoothed: f64,
    prev_smoothed: f64,
    initialized: bool,
    current_signal: Signal,
    alpha: f64,
}

impl OrderBookImbalanceStrategy {
    pub fn new(
        symbols: Vec<String>,
        smoothing_period: usize,
        entry_threshold: f64,
        exit_threshold: f64,
    ) -> Self {
        let alpha = 2.0 / (smoothing_period as f64 + 1.0);
        let mut state = HashMap::new();
        for sym in &symbols {
            state.insert(
                sym.clone(),
                OrderBookImbalanceState {
                    smoothed: 0.0,
                    prev_smoothed: 0.0,
                    initialized: false,
                    current_signal: Signal::None,
                    alpha,
                },
            );
        }
        Self {
            name: format!("OBImb_{}_{}_{}", smoothing_period, entry_threshold, exit_threshold),
            symbols: state,
            entry_threshold,
            exit_threshold,
        }
    }
}

impl TradingStrategy for OrderBookImbalanceStrategy {
    fn on_tick(&mut self, _symbol: &str, _bid: f64, _ask: f64, _volume: f64) -> Signal {
        Signal::None
    }

    fn on_bar(
        &mut self,
        _symbol: &str,
        _open: f64,
        _high: f64,
        _low: f64,
        _close: f64,
        _volume: f64,
        _timestamp_ns: i64,
    ) -> Signal {
        Signal::None
    }

    fn on_imbalance(&mut self, symbol: &str, imbalance: f64) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            if !state.initialized {
                state.smoothed = imbalance;
                state.prev_smoothed = imbalance;
                state.initialized = true;
                return Signal::None;
            }

            state.prev_smoothed = state.smoothed;
            state.smoothed = imbalance * state.alpha + state.smoothed * (1.0 - state.alpha);

            if state.smoothed > self.entry_threshold
                && state.prev_smoothed <= self.entry_threshold
                && state.current_signal != Signal::Buy
            {
                state.current_signal = Signal::Buy;
            } else if state.smoothed < -self.entry_threshold
                && state.prev_smoothed >= -self.entry_threshold
                && state.current_signal != Signal::Sell
            {
                state.current_signal = Signal::Sell;
            } else if state.current_signal == Signal::Buy
                && state.smoothed < self.exit_threshold
            {
                state.current_signal = Signal::Exit;
            } else if state.current_signal == Signal::Sell
                && state.smoothed > -self.exit_threshold
            {
                state.current_signal = Signal::Exit;
            }

            state.current_signal
        } else {
            Signal::None
        }
    }

    fn signal(&mut self, symbol: &str) -> Signal {
        self.symbols
            .get(symbol)
            .map(|s| s.current_signal)
            .unwrap_or(Signal::None)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ── Order Flow Strategy ──────────────────────────────────────────

pub struct OrderFlowStrategy {
    name: String,
    symbols: HashMap<String, OrderFlowState>,
    divergence_lookback: usize,
}

struct OrderFlowState {
    prices: Vec<f64>,
    deltas: Vec<f64>,     // per-tick delta increments
    idx: usize,
    filled: bool,
    current_signal: Signal,
}

impl OrderFlowStrategy {
    pub fn new(
        symbols: Vec<String>,
        divergence_lookback: usize,
        _divergence_threshold: f64,
    ) -> Self {
        let mut state = HashMap::new();
        for sym in &symbols {
            state.insert(
                sym.clone(),
                OrderFlowState {
                    prices: vec![0.0; divergence_lookback],
                    deltas: vec![0.0; divergence_lookback],
                    idx: 0,
                    filled: false,
                    current_signal: Signal::None,
                },
            );
        }
        Self {
            name: format!("OrderFlow_{}", divergence_lookback),
            symbols: state,
            divergence_lookback,
        }
    }
}

impl TradingStrategy for OrderFlowStrategy {
    fn on_tick(&mut self, _symbol: &str, _bid: f64, _ask: f64, _volume: f64) -> Signal {
        Signal::None
    }

    fn on_bar(
        &mut self,
        _symbol: &str,
        _open: f64,
        _high: f64,
        _low: f64,
        _close: f64,
        _volume: f64,
        _timestamp_ns: i64,
    ) -> Signal {
        Signal::None
    }

    fn on_orderflow(&mut self, symbol: &str, delta: f64, price: f64) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            // Store per-tick delta (not cumulative) so half-sums stay meaningful
            state.prices[state.idx] = price;
            state.deltas[state.idx] = delta;
            state.idx = (state.idx + 1) % self.divergence_lookback;

            if state.idx == 0 {
                state.filled = true;
            }

            if !state.filled {
                return Signal::None;
            }

            // Compare NET delta sum over each half, not min/max of cumulative
            let half = self.divergence_lookback / 2;

            let (pl1, ph1, sum1) = {
                let (mut pl, mut ph, mut s) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
                for i in 0..half {
                    let pi = state.prices[(state.idx + i) % self.divergence_lookback];
                    let di = state.deltas[(state.idx + i) % self.divergence_lookback];
                    if pi < pl { pl = pi; }
                    if pi > ph { ph = pi; }
                    s += di;
                }
                (pl, ph, s)
            };

            let (pl2, ph2, sum2) = {
                let (mut pl, mut ph, mut s) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
                for i in half..self.divergence_lookback {
                    let pi = state.prices[(state.idx + i) % self.divergence_lookback];
                    let di = state.deltas[(state.idx + i) % self.divergence_lookback];
                    if pi < pl { pl = pi; }
                    if pi > ph { ph = pi; }
                    s += di;
                }
                (pl, ph, s)
            };

            // Bullish divergence: price makes lower low, net delta is higher
            if pl2 < pl1 && sum2 > sum1 && state.current_signal != Signal::Buy {
                state.current_signal = Signal::Buy;
            }
            // Bearish divergence: price makes higher high, net delta is lower
            else if ph2 > ph1 && sum2 < sum1 && state.current_signal != Signal::Sell {
                state.current_signal = Signal::Sell;
            }
            // Exit when price-delta alignment resumes
            else if state.current_signal == Signal::Buy && !(pl2 < pl1 && sum2 > sum1) {
                state.current_signal = Signal::Exit;
            } else if state.current_signal == Signal::Sell && !(ph2 > ph1 && sum2 < sum1) {
                state.current_signal = Signal::Exit;
            }

            state.current_signal
        } else {
            Signal::None
        }
    }

    fn signal(&mut self, symbol: &str) -> Signal {
        self.symbols
            .get(symbol)
            .map(|s| s.current_signal)
            .unwrap_or(Signal::None)
    }

    fn name(&self) -> &str {
        &self.name
    }
}
