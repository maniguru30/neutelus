use std::collections::HashMap;
use std::sync::Mutex;

use crate::strategy::TradingStrategy;
use crate::types::Signal;

static ENGINES: Mutex<Option<EngineRegistry>> = Mutex::new(None);

pub(crate) struct EngineRegistry {
    engines: HashMap<i32, EngineInstance>,
    next_id: i32,
}

struct EngineInstance {
    strategy: Box<dyn TradingStrategy + Send>,
}

impl EngineRegistry {
    fn new() -> Self {
        Self {
            engines: HashMap::new(),
            next_id: 1,
        }
    }

    fn register(&mut self, strategy: Box<dyn TradingStrategy + Send>) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        self.engines.insert(id, EngineInstance { strategy });
        id
    }

    fn get_mut(&mut self, id: i32) -> Option<&mut EngineInstance> {
        self.engines.get_mut(&id)
    }

    fn remove(&mut self, id: i32) -> bool {
        self.engines.remove(&id).is_some()
    }
}

pub(crate) fn init_registry() {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in init_registry");
    if reg.is_none() {
        *reg = Some(EngineRegistry::new());
    }
}

pub(crate) fn create_engine(
    strategy: Box<dyn TradingStrategy + Send>,
) -> i32 {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in create_engine");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    registry.register(strategy)
}

pub(crate) fn destroy_engine(handle: i32) -> bool {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in destroy_engine");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    registry.remove(handle)
}

pub(crate) fn process_tick(handle: i32, symbol: &str, bid: f64, ask: f64, volume: f64) -> Signal {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in process_tick");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    if let Some(instance) = registry.get_mut(handle) {
        instance.strategy.on_tick(symbol, bid, ask, volume)
    } else {
        Signal::Error
    }
}

pub(crate) fn process_bar(
    handle: i32,
    symbol: &str,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    timestamp_ns: i64,
) -> Signal {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in process_bar");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    if let Some(instance) = registry.get_mut(handle) {
        instance
            .strategy
            .on_bar(symbol, open, high, low, close, volume, timestamp_ns)
    } else {
        Signal::Error
    }
}

pub(crate) fn process_imbalance(handle: i32, symbol: &str, imbalance: f64) -> Signal {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in process_imbalance");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    if let Some(instance) = registry.get_mut(handle) {
        instance.strategy.on_imbalance(symbol, imbalance)
    } else {
        Signal::Error
    }
}

pub(crate) fn process_orderflow(handle: i32, symbol: &str, delta: f64, volume: f64) -> Signal {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in process_orderflow");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    if let Some(instance) = registry.get_mut(handle) {
        instance.strategy.on_orderflow(symbol, delta, volume)
    } else {
        Signal::Error
    }
}

pub(crate) fn get_signal(handle: i32, symbol: &str) -> Signal {
    let mut reg = ENGINES
        .lock()
        .expect("ENGINES mutex poisoned in get_signal");
    let registry = reg.as_mut().expect("Engine registry not initialized");
    if let Some(instance) = registry.get_mut(handle) {
        instance.strategy.signal(symbol)
    } else {
        Signal::Error
    }
}
