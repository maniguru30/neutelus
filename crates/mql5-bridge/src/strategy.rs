use std::collections::HashMap;

use crate::types::Signal;

pub trait TradingStrategy: Send {
    fn on_imbalance(&mut self, _symbol: &str, _imbalance: f64) -> Signal {
        Signal::None
    }
    fn on_orderflow(&mut self, _symbol: &str, _delta: f64, _volume: f64) -> Signal {
        Signal::None
    }
    fn name(&self) -> &str;
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

    fn name(&self) -> &str {
        &self.name
    }
}

// ── Order Flow Strategy (Cumulative Delta) ───────────────────────

pub struct OrderFlowStrategy {
    name: String,
    symbols: HashMap<String, OrderFlowState>,
    lookback: usize,
    entry_threshold: f64,
    exit_threshold: f64,
}

struct OrderFlowState {
    deltas: Vec<f64>,
    idx: usize,
    filled: bool,
    current_signal: Signal,
    cum_delta: f64,
}

impl OrderFlowStrategy {
    pub fn new(
        symbols: Vec<String>,
        lookback: usize,
        divergence_threshold: f64,
    ) -> Self {
        let entry = if divergence_threshold > 0.0 { divergence_threshold } else { 5.0 };
        let exit = entry * 0.3;
        let mut state = HashMap::new();
        for sym in &symbols {
            state.insert(
                sym.clone(),
                OrderFlowState {
                    deltas: vec![0.0; lookback],
                    idx: 0,
                    filled: false,
                    current_signal: Signal::None,
                    cum_delta: 0.0,
                },
            );
        }
        Self {
            name: format!("OrderFlow_{}", lookback),
            symbols: state,
            lookback,
            entry_threshold: entry,
            exit_threshold: exit,
        }
    }
}

impl TradingStrategy for OrderFlowStrategy {
    fn on_orderflow(&mut self, symbol: &str, delta: f64, _price: f64) -> Signal {
        if let Some(state) = self.symbols.get_mut(symbol) {
            state.cum_delta -= state.deltas[state.idx];
            state.deltas[state.idx] = delta;
            state.cum_delta += delta;
            state.idx = (state.idx + 1) % self.lookback;

            if state.idx == 0 {
                state.filled = true;
            }

            if !state.filled {
                return Signal::None;
            }

            match state.current_signal {
                Signal::None | Signal::Exit => {
                    if state.cum_delta > self.entry_threshold {
                        state.current_signal = Signal::Buy;
                    } else if state.cum_delta < -self.entry_threshold {
                        state.current_signal = Signal::Sell;
                    }
                }
                Signal::Buy => {
                    if state.cum_delta < self.exit_threshold {
                        state.current_signal = Signal::Exit;
                    }
                }
                Signal::Sell => {
                    if state.cum_delta > -self.exit_threshold {
                        state.current_signal = Signal::Exit;
                    }
                }
                _ => {}
            }

            state.current_signal
        } else {
            Signal::None
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
