use std::ffi::CString;
use std::os::raw::c_char;
use std::panic;

use engine::{
    create_engine, destroy_engine, get_signal, init_registry, process_bar, process_imbalance,
    process_orderflow, process_tick,
};
use strategy::{
    EMACrossStrategy, OrderBookImbalanceStrategy, OrderFlowStrategy, RSIMeanReversionStrategy,
};
use types::{cstr_to_str, Signal};

mod engine;
pub mod strategy;
pub mod types;

fn catch_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> Result<R, ()> {
    match panic::catch_unwind(f) {
        Ok(r) => Ok(r),
        Err(_) => Err(()),
    }
}

struct StrategyConfig {
    strategy_type: String,
    symbols: Vec<String>,
    fast_period: usize,
    slow_period: usize,
    oversold: f64,
    overbought: f64,
    smoothing_period: usize,
    entry_threshold: f64,
    exit_threshold: f64,
    divergence_lookback: usize,
    divergence_threshold: f64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            strategy_type: "ema_cross".to_string(),
            symbols: vec!["EURUSD".to_string()],
            fast_period: 12,
            slow_period: 26,
            oversold: 30.0,
            overbought: 70.0,
            smoothing_period: 20,
            entry_threshold: 0.3,
            exit_threshold: 0.1,
            divergence_lookback: 50,
            divergence_threshold: 0.0,
        }
    }
}

fn parse_config(config_json: &str) -> StrategyConfig {
    let mut cfg = StrategyConfig::default();

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(config_json) {
        if let Some(s) = v.get("strategy").and_then(|s| s.as_str()) {
            cfg.strategy_type = s.to_string();
        }
        if let Some(a) = v.get("symbols").and_then(|s| s.as_array()) {
            let syms: Vec<_> = a.iter().filter_map(|s| s.as_str().map(String::from)).collect();
            if !syms.is_empty() {
                cfg.symbols = syms;
            }
        }
        if let Some(p) = v.get("fast_period").and_then(|p| p.as_u64()) {
            cfg.fast_period = p as usize;
        }
        if let Some(p) = v.get("slow_period").and_then(|p| p.as_u64()) {
            cfg.slow_period = p as usize;
        }
        if let Some(p) = v.get("oversold").and_then(|p| p.as_f64()) {
            cfg.oversold = p;
        }
        if let Some(p) = v.get("overbought").and_then(|p| p.as_f64()) {
            cfg.overbought = p;
        }
        if let Some(p) = v.get("smoothing_period").and_then(|p| p.as_u64()) {
            cfg.smoothing_period = p as usize;
        }
        if let Some(p) = v.get("entry_threshold").and_then(|p| p.as_f64()) {
            cfg.entry_threshold = p;
        }
        if let Some(p) = v.get("exit_threshold").and_then(|p| p.as_f64()) {
            cfg.exit_threshold = p;
        }
        if let Some(p) = v.get("divergence_lookback").and_then(|p| p.as_u64()) {
            cfg.divergence_lookback = p as usize;
        }
        if let Some(p) = v.get("divergence_threshold").and_then(|p| p.as_f64()) {
            cfg.divergence_threshold = p;
        }
    }

    cfg
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_init(config_json: *const c_char) -> i32 {
    catch_panic(|| {
        init_registry();
        let config = unsafe { cstr_to_str(config_json) };
        let cfg = parse_config(config);
        let syms = cfg.symbols.clone();

        let strategy: Box<dyn strategy::TradingStrategy + Send> = match cfg.strategy_type.as_str() {
            "rsi" => Box::new(RSIMeanReversionStrategy::new(
                syms, cfg.fast_period, cfg.oversold, cfg.overbought,
            )),
            "orderbook_imbalance" | "obi" => Box::new(OrderBookImbalanceStrategy::new(
                syms,
                cfg.smoothing_period,
                cfg.entry_threshold,
                cfg.exit_threshold,
            )),
            "orderflow" | "of" => Box::new(OrderFlowStrategy::new(
                syms,
                cfg.divergence_lookback,
                cfg.divergence_threshold,
            )),
            "ema_cross" | _ => Box::new(EMACrossStrategy::new(syms, cfg.fast_period, cfg.slow_period)),
        };

        create_engine(strategy)
    })
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_deinit(handle: i32) {
    let _ = catch_panic(|| {
        destroy_engine(handle);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_on_tick(
    handle: i32,
    symbol: *const c_char,
    bid: f64,
    ask: f64,
    volume: f64,
) -> i32 {
    catch_panic(|| {
        let sym = unsafe { cstr_to_str(symbol) };
        let sig = process_tick(handle, sym, bid, ask, volume);
        sig as i32
    })
    .unwrap_or(Signal::Error as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_on_bar(
    handle: i32,
    symbol: *const c_char,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    timestamp_ns: i64,
) -> i32 {
    catch_panic(|| {
        let sym = unsafe { cstr_to_str(symbol) };
        let sig = process_bar(handle, sym, open, high, low, close, volume, timestamp_ns);
        sig as i32
    })
    .unwrap_or(Signal::Error as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_on_imbalance(
    handle: i32,
    symbol: *const c_char,
    imbalance: f64,
) -> i32 {
    catch_panic(|| {
        let sym = unsafe { cstr_to_str(symbol) };
        let sig = process_imbalance(handle, sym, imbalance);
        sig as i32
    })
    .unwrap_or(Signal::Error as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_on_orderflow(
    handle: i32,
    symbol: *const c_char,
    delta: f64,
    volume: f64,
) -> i32 {
    catch_panic(|| {
        let sym = unsafe { cstr_to_str(symbol) };
        let sig = process_orderflow(handle, sym, delta, volume);
        sig as i32
    })
    .unwrap_or(Signal::Error as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_signal(handle: i32, symbol: *const c_char) -> i32 {
    catch_panic(|| {
        let sym = unsafe { cstr_to_str(symbol) };
        let sig = get_signal(handle, sym);
        sig as i32
    })
    .unwrap_or(Signal::Error as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_version() -> *const c_char {
    let version = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    version.into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn nt_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
