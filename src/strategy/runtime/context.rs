use crate::models::Candle;

pub struct EvalContext<'a> {
    pub candles: &'a [Candle],
    pub current: &'a Candle,
}

impl<'a> EvalContext<'a> {
    pub fn new(candles: &'a [Candle]) -> Option<Self> {
        candles.last().map(|current| Self { candles, current })
    }
}
