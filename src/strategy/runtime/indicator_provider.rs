use std::collections::HashMap;

use crate::indicators::{atr, ema, ma, rsi};
use crate::models::Candle;
use crate::strategy::dsl::IndicatorKind;

pub trait IndicatorProvider: Send + Sync {
    fn get(&mut self, kind: &IndicatorKind, period: usize, candles: &[Candle]) -> Option<f64>;

    fn clear_cache(&mut self) {}

    fn advance(&mut self, _candle: &Candle) {}
}

#[derive(Debug, Default)]
pub struct FullRecomputeProvider {
    cache: HashMap<(IndicatorKind, usize), f64>,
}

impl FullRecomputeProvider {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
}

impl IndicatorProvider for FullRecomputeProvider {
    fn get(&mut self, kind: &IndicatorKind, period: usize, candles: &[Candle]) -> Option<f64> {
        if period == 0 || candles.is_empty() {
            return None;
        }

        let key = (kind.clone(), period);
        if let Some(value) = self.cache.get(&key) {
            return Some(*value);
        }

        let value = latest_indicator_value(kind, period, candles)?;
        self.cache.insert(key, value);
        Some(value)
    }

    fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

fn latest_indicator_value(kind: &IndicatorKind, period: usize, candles: &[Candle]) -> Option<f64> {
    let values = match kind {
        IndicatorKind::Ma => ma(candles, period),
        IndicatorKind::Ema => ema(candles, period),
        IndicatorKind::Rsi => rsi(candles, period),
        IndicatorKind::Atr => atr(candles, period),
        IndicatorKind::Vwap
        | IndicatorKind::BbUpper
        | IndicatorKind::BbLower
        | IndicatorKind::BbMid => return None,
    };

    values.last().copied().filter(|value| value.is_finite())
}
