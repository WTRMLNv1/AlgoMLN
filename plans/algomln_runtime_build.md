# AlgoMLN — Build Runtime Modules
## Codex Task: Implement Phase C and D from the architecture spec

---

## Context

Phases A and B are complete and tested:
- AST (`ast.rs`)
- Lexer (`lexer.rs`) — 68 tests passing
- Parser (`parser.rs`)
- Validator (`validator.rs`)
- ExecutionTarget trait (`target.rs`)
- PaperBroker (`paper.rs`)
- OrderBuilder (`order_builder.rs`)

The following modules do not exist yet and must be built now:

```
src-tauri/src/strategy/runtime/trigger_state.rs
src-tauri/src/strategy/runtime/cross.rs
src-tauri/src/strategy/runtime/indicator_provider.rs
src-tauri/src/strategy/runtime/context.rs
src-tauri/src/strategy/runtime/engine.rs
```

And one integration module:

```
src-tauri/src/strategy/tests/backtest_integration.rs
```

---

## Guiding Principle

Correctness and determinism over speed.
A strategy must produce identical results every time it runs on the same candle data.
No randomness. No non-deterministic data structures in evaluation paths.
Use `BTreeMap` instead of `HashMap` anywhere the iteration order could affect output.

---

## Module 1: `trigger_state.rs`

Tracks whether a condition was true on the previous candle for each rule.
Fires only on `false → true` transitions.

```rust
pub struct TriggerStateMap {
    states: BTreeMap<String, bool>,  // rule_id → was_true_last_candle
}

impl TriggerStateMap {
    pub fn new() -> Self

    /// Returns true only when transitioning from false to true.
    /// Always updates the stored state, even on error (pass false on error).
    pub fn should_fire(&mut self, rule_id: &str, is_true_now: bool) -> bool {
        let was_true = *self.states.get(rule_id).unwrap_or(&false);
        self.states.insert(rule_id.to_string(), is_true_now);
        !was_true && is_true_now
    }
}
```

Add `#[cfg(test)]` block at the bottom with these tests:

- `fires_on_false_to_true` — first call with `true` must return `true`
- `does_not_fire_on_true_to_true` — second consecutive `true` must return `false`
- `does_not_fire_on_true_to_false` — transition to false must return `false`
- `does_not_fire_on_false_to_false` — staying false must return `false`
- `resets_and_fires_again_after_false` — after going false, the next `true` must fire again
- `independent_state_per_rule` — `rule_0` state must not affect `rule_1` state

---

## Module 2: `cross.rs`

Detects when one value crosses above or below another between consecutive candles.
Requires previous candle values to be stored per rule.

```rust
pub struct CrossDetector {
    prev_values: BTreeMap<CrossStateKey, f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrossStateKey {
    pub rule_id: String,
    pub side: CrossSide,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrossSide { Fast, Slow }

impl CrossDetector {
    pub fn new() -> Self

    /// true only when: fast_prev <= slow_prev AND fast_curr > slow_curr
    /// Returns false if no previous values exist (first candle).
    pub fn is_cross_above(&self, rule_id: &str, fast_curr: f64, slow_curr: f64) -> bool

    /// true only when: fast_prev >= slow_prev AND fast_curr < slow_curr
    /// Returns false if no previous values exist (first candle).
    pub fn is_cross_below(&self, rule_id: &str, fast_curr: f64, slow_curr: f64) -> bool

    /// Must be called once per cross rule per candle, AFTER all rules are evaluated.
    pub fn update(&mut self, rule_id: &str, fast_curr: f64, slow_curr: f64)
}
```

**Critical ordering rule:** `update()` is called in a separate pass after all rules finish evaluating on a candle. Never inside the per-rule evaluation loop. This ensures all rules see consistent previous-candle values during one cycle.

Add `#[cfg(test)]` block with these tests:

- `returns_false_with_no_previous_values` — both methods return false on first call
- `detects_cross_above_on_exact_candle` — fast goes from below-or-equal to above
- `does_not_fire_cross_above_after_crossover` — subsequent candle where fast stays above must return false
- `detects_cross_below_on_exact_candle` — fast goes from above-or-equal to below
- `does_not_fire_cross_below_after_crossover` — symmetric
- `independent_state_per_rule` — rule_0 prev values must not affect rule_1

---

## Module 3: `indicator_provider.rs`

Abstracts indicator computation so the engine does not call indicator functions directly.
v1 uses full recomputation. The trait allows future incremental implementations.

```rust
pub trait IndicatorProvider: Send + Sync {
    /// Returns the most recent indicator value for the given (kind, period).
    /// Returns None if candle history is shorter than the period.
    fn get(&mut self, kind: &IndicatorKind, period: usize, candles: &[Candle]) -> Option<f64>;

    /// Called after each evaluation cycle. No-op for FullRecomputeProvider.
    fn advance(&mut self, _candle: &Candle) {}
}

/// v1: recomputes from full history on every call.
/// Within-cycle cache prevents recomputing the same (kind, period) twice.
pub struct FullRecomputeProvider {
    cache: HashMap<(IndicatorKind, usize), f64>,
}

impl FullRecomputeProvider {
    pub fn new() -> Self
    /// Clear the within-cycle cache. Must be called at the start of each on_candle.
    pub fn clear_cache(&mut self)
}
```

The cache key is `(IndicatorKind, period)`. On the first call for a given key within a cycle, compute and store. On subsequent calls for the same key, return the cached value. Clear at the start of each cycle via `clear_cache()`.

Indicator dispatch inside `get()`:

```rust
match kind {
    IndicatorKind::Ema     => crate::indicators::ema(candles, period).last().copied(),
    IndicatorKind::Ma      => crate::indicators::ma(candles, period).last().copied(),
    IndicatorKind::Rsi     => crate::indicators::rsi(candles, period).last().copied(),
    IndicatorKind::Atr     => crate::indicators::atr(candles, period).last().copied(),
    IndicatorKind::Vwap    => crate::indicators::vwap(candles, period).last().copied(),
    IndicatorKind::BbUpper => crate::indicators::bollinger_bands(candles, period).map(|b| b.last_upper()),
    IndicatorKind::BbLower => crate::indicators::bollinger_bands(candles, period).map(|b| b.last_lower()),
    IndicatorKind::BbMid   => crate::indicators::bollinger_bands(candles, period).map(|b| b.last_mid()),
}
```

Adjust the paths above to match the actual module structure in the repo. Do not rename or re-implement existing indicator functions.

---

## Module 4: `context.rs`

A lightweight per-cycle view. Owns nothing, borrows the candle slice.

```rust
pub struct EvalContext<'a> {
    pub candles: &'a [Candle],
    pub current: &'a Candle,    // always candles.last().unwrap()
}

impl<'a> EvalContext<'a> {
    pub fn new(candles: &'a [Candle]) -> Option<Self> {
        candles.last().map(|current| Self { candles, current })
    }
}
```

Returns `None` if the candle slice is empty. The engine checks for `None` and returns early.

---

## Module 5: `engine.rs`

The main evaluation loop. Wires all previous modules together.

### Imports needed

`TriggerStateMap`, `CrossDetector`, `IndicatorProvider`, `FullRecomputeProvider`,
`EvalContext`, `ExecutionTarget`, `PaperBroker`, `order_builder`,
`StrategyNode`, `RuleNode`, `ConditionNode`, `ExprNode`, `ActionNode`, `IndicatorCall`, `PriceField`,
`StrategyInstance`, `StrategyStatus`, `StrategyLogger`, `LogEntryKind`, `IndicatorSnapshot`

### EvalError

```rust
#[derive(Debug, Clone)]
pub enum EvalError {
    InsufficientData { indicator: IndicatorKind, period: usize, available: usize },
    NotYetImplemented(&'static str),
    OrderBuildFailed(String),
    EmptyCandles,
}
```

### StrategyEngine struct

```rust
pub struct StrategyEngine {
    pub instance: StrategyInstance,
    cross_detector: CrossDetector,
    trigger_state: TriggerStateMap,
    indicator_provider: Box<dyn IndicatorProvider>,
    logger: StrategyLogger,
}

impl StrategyEngine {
    pub fn new(instance: StrategyInstance) -> Self {
        Self {
            cross_detector: CrossDetector::new(),
            trigger_state: TriggerStateMap::new(),
            indicator_provider: Box::new(FullRecomputeProvider::new()),
            logger: StrategyLogger::new(instance.id.clone()),
            instance,
        }
    }

    pub async fn on_candle(&mut self, candles: &[Candle]) -> Vec<LogEntry>
}
```

### `on_candle` implementation (follow this order exactly)

```
1. If instance.status is Paused or Stopped → return vec![]
2. Call indicator_provider.clear_cache()
3. Build EvalContext. If None (empty candles) → return vec![]
4. Collect cross-rule state BEFORE evaluation (for update pass after)
5. For each rule in instance.strategy.rules:
   a. Call eval_condition(rule, &ctx)
      On EvalError: log EvalError, call trigger_state.should_fire(rule_id, false), continue
   b. Call trigger_state.should_fire(rule_id, condition_result) → fired
   c. Log ConditionEvaluated { result, prev_state, fired, indicator_snapshots }
   d. If fired:
      i.   Log RuleFired
      ii.  Call order_builder::build_order(action, symbol, current_price, position, rule_id)
           On error: log EvalError, continue
      iii. Log OrderSubmitted
      iv.  Call instance.execution_target.execute(order).await
      v.   Log OrderExecuted or OrderFailed
6. For each rule with CrossAbove or CrossBelow condition:
   evaluate the fast and slow exprs again (using the cached provider — no recomputation)
   call cross_detector.update(rule_id, fast_val, slow_val)
7. Call indicator_provider.advance(ctx.current)
8. Return logger.drain_entries()
```

### Expression evaluator

```rust
fn eval_expr(
    expr: &ExprNode,
    ctx: &EvalContext,
    provider: &mut dyn IndicatorProvider,
) -> Result<f64, EvalError>
```

- `Literal(v)` → `Ok(v)`
- `PriceField(f)` → return the matching field from `ctx.current`
- `Indicator(call)` → call `provider.get(&call.kind, call.period, ctx.candles)`. If `None` → `Err(EvalError::InsufficientData { ... })`

### Condition evaluator

```rust
fn eval_condition(
    condition: &ConditionNode,
    ctx: &EvalContext,
    provider: &mut dyn IndicatorProvider,
    cross_detector: &CrossDetector,
    rule_id: &str,
) -> Result<bool, EvalError>
```

- `Comparison { left, op, right }` → eval both, apply op
- `CrossAbove { fast, slow }` → eval fast and slow, call `cross_detector.is_cross_above(rule_id, fast_val, slow_val)`
- `CrossBelow { fast, slow }` → eval fast and slow, call `cross_detector.is_cross_below(rule_id, fast_val, slow_val)`
- `And(a, b)` → short-circuit: eval a; if false return false without evaluating b
- `Or(a, b)` → short-circuit: eval a; if true return true without evaluating b
- `Not(inner)` → eval inner, return `!result`
- `InPosition` → `Err(EvalError::NotYetImplemented("in_position"))`
- `TimeWindow { .. }` → `Err(EvalError::NotYetImplemented("between"))`

### Engine integration tests

Add `#[cfg(test)]` at the bottom of `engine.rs` with these tests.
Each test builds a full pipeline: parse → validate → build StrategyInstance → build StrategyEngine.

**Helper to build a minimal Candle:**
```rust
fn candle(close: f64) -> Candle {
    Candle { open: close, high: close, low: close, close, volume: 1000.0, timestamp: Utc::now() }
}
```

**Helper to count executed trades from log entries:**
```rust
fn count_trades(logs: &[LogEntry]) -> usize {
    logs.iter().filter(|e| matches!(e.kind, LogEntryKind::OrderExecuted { .. })).count()
}
```

**Test 1: `fires_exactly_once_on_condition_trigger`**

Strategy: `WHEN close > 105 / BUY 1`
Candles: 100, 101, 102, 103, 104, 105, 106, 107, 108
Feed candles one at a time (slice grows: `&candles[..1]`, `&candles[..2]`, etc.)
Expected: exactly 1 total trade

**Test 2: `idiot_test_fires_only_once`**

Strategy: `WHEN close > 0 / BUY 1`
Candles: 1.0 through 20.0
Expected: exactly 1 total trade
This is the most important test. If trigger state is broken, this returns 20.

**Test 3: `fires_again_after_condition_resets`**

Strategy: `WHEN close > 105 / BUY 1`
Candles: 100.0, 106.0, 100.0, 106.0
Expected: exactly 2 trades (one per crossing-above event)

**Test 4: `cross_above_fires_exactly_once`**

Strategy: `WHEN cross_above(ma(2), ma(5)) / BUY 1`
Candles: 50.0, 49.0, 48.0, 47.0, 46.0, 45.0, 90.0, 92.0, 93.0, 94.0
(descending trend, then a sharp spike forces MA(2) above MA(5))
Expected: exactly 1 trade

**Test 5: `engine_skips_evaluation_when_paused`**

Strategy: `WHEN close > 0 / BUY 1`
Set `instance.status = StrategyStatus::Paused`
Feed 5 candles
Expected: 0 trades, empty log

---

## Module 6: Backtest integration (`tests/backtest_integration.rs`)

Create `src-tauri/src/strategy/tests/backtest_integration.rs`.
Add `mod tests;` to `src-tauri/src/strategy/tests/mod.rs` or create it if it does not exist.

This file tests `run_backtest_internal` — the internal synchronous function that the Tauri command calls.

If `run_backtest_internal` does not exist yet, implement it in `src-tauri/src/commands/strategy.rs`:

```rust
pub async fn run_backtest_internal(
    node: StrategyNode,
    symbol: String,
    candles: Vec<Candle>,
    initial_cash: f64,
) -> Result<BacktestResult, String>
```

It should:
1. Validate `node`. Return error string if validation fails.
2. Create a `PaperBroker` for the symbol with `initial_cash`.
3. Create a `StrategyInstance` with status `Running`.
4. Create a `StrategyEngine`.
5. Feed candles one at a time: for `i in 1..=candles.len()`, call `engine.on_candle(&candles[..i]).await`.
6. Collect all logs.
7. Return `BacktestResult`.

**Backtest tests:**

**Test 1: `backtest_known_output`**

Strategy: `WHEN close > 105 / BUY 1`
Candles: 100.0 through 108.0 (9 candles)
Expected: 1 trade, 9 candles processed

**Test 2: `backtest_is_deterministic`**

Strategy: `WHEN close > 105 / BUY 1 / WHEN close < 102 / SELL ALL`
Candles: 98.0 through 110.0
Run twice with identical inputs.
Assert: trade count, PnL, final cash, and each trade's price/quantity/side are identical.

**Test 3: `backtest_buy_sell_pnl_is_correct`**

Strategy: `WHEN close > 105 / BUY 1 / WHEN close < 95 / SELL ALL`
Candles: 100.0, 110.0, 90.0
Expected: 2 trades, total_realized_pnl = -20.0

---

## After Implementation

Run:

```bash
cargo test --lib
```

Expected new passing tests (in addition to the existing 68):

```
strategy::runtime::trigger_state::tests::fires_on_false_to_true
strategy::runtime::trigger_state::tests::does_not_fire_on_true_to_true
strategy::runtime::trigger_state::tests::does_not_fire_on_true_to_false
strategy::runtime::trigger_state::tests::does_not_fire_on_false_to_false
strategy::runtime::trigger_state::tests::resets_and_fires_again_after_false
strategy::runtime::trigger_state::tests::independent_state_per_rule

strategy::runtime::cross::tests::returns_false_with_no_previous_values
strategy::runtime::cross::tests::detects_cross_above_on_exact_candle
strategy::runtime::cross::tests::does_not_fire_cross_above_after_crossover
strategy::runtime::cross::tests::detects_cross_below_on_exact_candle
strategy::runtime::cross::tests::does_not_fire_cross_below_after_crossover
strategy::runtime::cross::tests::independent_state_per_rule

strategy::runtime::engine::tests::fires_exactly_once_on_condition_trigger
strategy::runtime::engine::tests::idiot_test_fires_only_once
strategy::runtime::engine::tests::fires_again_after_condition_resets
strategy::runtime::engine::tests::cross_above_fires_exactly_once
strategy::runtime::engine::tests::engine_skips_evaluation_when_paused

strategy::tests::backtest_integration::backtest_known_output
strategy::tests::backtest_integration::backtest_is_deterministic
strategy::tests::backtest_integration::backtest_buy_sell_pnl_is_correct
```

Total target: 68 existing + ~21 new = ~89 passing, 0 failed.

If the idiot test or the determinism test fails, stop and fix before anything else.

---

## What You Must Not Do

- Do not re-implement or rename existing indicator functions.
- Do not connect to Dhan or any network in any test.
- Do not use `HashMap` in any path where iteration order could affect evaluation results — use `BTreeMap`.
- Do not write Tauri commands yet. Only the internal `run_backtest_internal` function is needed now.
- Do not skip the idiot test.
