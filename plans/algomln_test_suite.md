# AlgoMLN — Strategy Engine Test Suite
## Codex Task: Write and run all tests for the strategy engine

---

## Context

The strategy engine has been implemented according to the architecture spec.
Your job is to write the complete test suite and verify everything works correctly.

Do not touch live data. Do not connect to Dhan. Do not modify any existing implementation files.
Write tests only.

---

## File Locations

All tests go in the following files, each in a `#[cfg(test)]` block at the bottom of the file:

```
src-tauri/src/strategy/dsl/lexer.rs         → lexer tests
src-tauri/src/strategy/dsl/parser.rs        → parser tests
src-tauri/src/strategy/dsl/validator.rs     → validator tests
src-tauri/src/strategy/runtime/cross.rs     → cross detector tests
src-tauri/src/strategy/runtime/trigger_state.rs  → trigger state tests
src-tauri/src/strategy/execution/paper.rs   → paper broker tests
src-tauri/src/strategy/runtime/engine.rs    → engine integration tests
```

One additional file for backtest integration:

```
src-tauri/src/strategy/tests/backtest_integration.rs
```

Create this file if it does not exist. Add it to `mod.rs` under `#[cfg(test)]`.

---

## Layer 1 — Unit Tests

### Lexer tests (`lexer.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_simple_strategy() {
        let src = "WHEN rsi(14) < 30\nBUY 5";
        let tokens = Lexer::new(src).tokenize().unwrap();
        // assert token sequence: When, Rsi, LParen, Integer(14), RParen, Lt, Integer(30), Newline, Buy, Integer(5), Eof
    }

    #[test]
    fn tokenizes_sell_all() {
        let src = "WHEN close > 100\nSELL ALL";
        let tokens = Lexer::new(src).tokenize().unwrap();
        // assert SellAll token sequence includes: Sell, All
    }

    #[test]
    fn skips_comments() {
        let src = "# this is a comment\nWHEN close > 100\nBUY 1";
        let tokens = Lexer::new(src).tokenize().unwrap();
        // first meaningful token should be When, not a comment token
        assert_eq!(tokens[0].kind, TokenKind::When);
    }

    #[test]
    fn skips_blank_lines() {
        let src = "\n\nWHEN close > 100\nBUY 1";
        let tokens = Lexer::new(src).tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::When);
    }

    #[test]
    fn distinguishes_lte_from_lt() {
        let src = "WHEN close <= 100\nBUY 1";
        let tokens = Lexer::new(src).tokenize().unwrap();
        let ops: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
        assert!(ops.contains(&&TokenKind::Lte));
        assert!(!ops.contains(&&TokenKind::Lt));
    }

    #[test]
    fn integer_vs_float() {
        let src = "WHEN close > 105.5\nBUY 1";
        let tokens = Lexer::new(src).tokenize().unwrap();
        let has_float = tokens.iter().any(|t| matches!(t.kind, TokenKind::Number(_)));
        assert!(has_float);
    }

    #[test]
    fn unknown_character_produces_lex_error() {
        let src = "WHEN close @ 100\nBUY 1";
        let result = Lexer::new(src).tokenize();
        assert!(result.is_err());
    }

    #[test]
    fn cross_above_tokenizes_correctly() {
        let src = "WHEN cross_above(ema(20), ema(50))\nBUY 10";
        let tokens = Lexer::new(src).tokenize().unwrap();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::CrossAbove));
    }
}
```

### Parser tests (`parser.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::dsl::lexer::Lexer;

    fn parse(src: &str) -> Result<StrategyNode, ParseError> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse()
    }

    #[test]
    fn parses_simple_rsi_rule() {
        let node = parse("WHEN rsi(14) < 30\nBUY 5").unwrap();
        assert_eq!(node.rules.len(), 1);
        assert_eq!(node.rules[0].id, "rule_0");
        assert!(matches!(node.rules[0].action, ActionNode::Buy { quantity: 5 }));
    }

    #[test]
    fn assigns_sequential_rule_ids() {
        let src = "WHEN rsi(14) < 30\nBUY 5\n\nWHEN rsi(14) > 70\nSELL ALL";
        let node = parse(src).unwrap();
        assert_eq!(node.rules[0].id, "rule_0");
        assert_eq!(node.rules[1].id, "rule_1");
    }

    #[test]
    fn sell_all_is_sell_all_not_sell_quantity() {
        let node = parse("WHEN close > 100\nSELL ALL").unwrap();
        assert!(matches!(node.rules[0].action, ActionNode::SellAll));
    }

    #[test]
    fn parses_cross_above() {
        let node = parse("WHEN cross_above(ema(20), ema(50))\nBUY 10").unwrap();
        assert!(matches!(
            node.rules[0].condition,
            ConditionNode::CrossAbove { .. }
        ));
    }

    #[test]
    fn parses_and_condition() {
        let src = "WHEN ema(9) > ema(21) AND rsi(14) < 60\nBUY 5";
        let node = parse(src).unwrap();
        assert!(matches!(node.rules[0].condition, ConditionNode::And(_, _)));
    }

    #[test]
    fn parses_not_condition() {
        let src = "WHEN NOT (rsi(14) > 70)\nBUY 5";
        let node = parse(src).unwrap();
        assert!(matches!(node.rules[0].condition, ConditionNode::Not(_)));
    }

    #[test]
    fn parse_error_on_missing_action() {
        let result = parse("WHEN rsi(14) < 30");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_on_incomplete_comparison() {
        let result = parse("WHEN rsi(14) <\nBUY 5");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_on_empty_input() {
        let result = parse("");
        assert!(result.is_err());
    }

    #[test]
    fn deterministic_ids_across_reparses() {
        let src = "WHEN rsi(14) < 30\nBUY 5";
        let node1 = parse(src).unwrap();
        let node2 = parse(src).unwrap();
        assert_eq!(node1.rules[0].id, node2.rules[0].id);
    }
}
```

### Validator tests (`validator.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_rsi_rule(period: usize, qty: usize) -> RuleNode {
        RuleNode {
            id: "rule_0".to_string(),
            condition: ConditionNode::Comparison {
                left: ExprNode::Indicator(IndicatorCall { kind: IndicatorKind::Rsi, period }),
                op: CompareOp::Lt,
                right: ExprNode::Literal(30.0),
            },
            action: ActionNode::Buy { quantity: qty },
        }
    }

    #[test]
    fn valid_strategy_has_no_errors() {
        let strategy = StrategyNode {
            name: "test".to_string(),
            rules: vec![make_rsi_rule(14, 5)],
        };
        let errors = AstValidator::validate(&strategy);
        assert!(errors.is_empty());
    }

    #[test]
    fn rejects_empty_strategy() {
        let strategy = StrategyNode { name: "test".to_string(), rules: vec![] };
        let errors = AstValidator::validate(&strategy);
        assert!(errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::EmptyStrategy)));
    }

    #[test]
    fn rejects_zero_period() {
        let strategy = StrategyNode {
            name: "test".to_string(),
            rules: vec![make_rsi_rule(0, 5)],
        };
        let errors = AstValidator::validate(&strategy);
        assert!(errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::InvalidPeriod { .. })));
    }

    #[test]
    fn rejects_zero_quantity() {
        let strategy = StrategyNode {
            name: "test".to_string(),
            rules: vec![make_rsi_rule(14, 0)],
        };
        let errors = AstValidator::validate(&strategy);
        assert!(errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::InvalidQuantity)));
    }

    #[test]
    fn rejects_cross_with_literal_operand() {
        let rule = RuleNode {
            id: "rule_0".to_string(),
            condition: ConditionNode::CrossAbove {
                fast: ExprNode::Literal(30.0),  // invalid: literal can't cross
                slow: ExprNode::Indicator(IndicatorCall { kind: IndicatorKind::Ema, period: 20 }),
            },
            action: ActionNode::Buy { quantity: 1 },
        };
        let strategy = StrategyNode { name: "test".to_string(), rules: vec![rule] };
        let errors = AstValidator::validate(&strategy);
        assert!(errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::CrossWithLiteral)));
    }

    #[test]
    fn collects_all_errors_not_just_first() {
        // strategy with both zero period and zero quantity
        let strategy = StrategyNode {
            name: "test".to_string(),
            rules: vec![make_rsi_rule(0, 0)],
        };
        let errors = AstValidator::validate(&strategy);
        assert!(errors.len() >= 2);
    }
}
```

### TriggerState tests (`trigger_state.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_on_false_to_true() {
        let mut state = TriggerStateMap::new();
        assert!(state.should_fire("rule_0", true));
    }

    #[test]
    fn does_not_fire_on_true_to_true() {
        let mut state = TriggerStateMap::new();
        state.should_fire("rule_0", true);   // transition to true
        assert!(!state.should_fire("rule_0", true));  // stays true, no fire
    }

    #[test]
    fn does_not_fire_on_true_to_false() {
        let mut state = TriggerStateMap::new();
        state.should_fire("rule_0", true);
        assert!(!state.should_fire("rule_0", false));
    }

    #[test]
    fn does_not_fire_on_false_to_false() {
        let mut state = TriggerStateMap::new();
        assert!(!state.should_fire("rule_0", false));
    }

    #[test]
    fn resets_and_fires_again_after_false() {
        let mut state = TriggerStateMap::new();
        state.should_fire("rule_0", true);   // fires
        state.should_fire("rule_0", false);  // resets
        assert!(state.should_fire("rule_0", true));  // fires again
    }

    #[test]
    fn independent_state_per_rule() {
        let mut state = TriggerStateMap::new();
        state.should_fire("rule_0", true);
        // rule_1 is still at false, so this should fire
        assert!(state.should_fire("rule_1", true));
    }
}
```

### CrossDetector tests (`cross.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_false_with_no_previous_values() {
        let detector = CrossDetector::new();
        assert!(!detector.is_cross_above("rule_0", 50.0, 40.0));
        assert!(!detector.is_cross_below("rule_0", 40.0, 50.0));
    }

    #[test]
    fn detects_cross_above_on_exact_candle() {
        let mut detector = CrossDetector::new();
        // prev: fast(48) <= slow(50). curr: fast(52) > slow(50) → cross above
        detector.update("rule_0", 48.0, 50.0);
        assert!(detector.is_cross_above("rule_0", 52.0, 50.0));
    }

    #[test]
    fn does_not_fire_cross_above_after_crossover() {
        let mut detector = CrossDetector::new();
        detector.update("rule_0", 48.0, 50.0);
        // crossover candle
        assert!(detector.is_cross_above("rule_0", 52.0, 50.0));
        // update to "already above" state
        detector.update("rule_0", 52.0, 50.0);
        // next candle, still above — must not fire again
        assert!(!detector.is_cross_above("rule_0", 55.0, 50.0));
    }

    #[test]
    fn detects_cross_below_on_exact_candle() {
        let mut detector = CrossDetector::new();
        // prev: fast(52) >= slow(50). curr: fast(48) < slow(50) → cross below
        detector.update("rule_0", 52.0, 50.0);
        assert!(detector.is_cross_below("rule_0", 48.0, 50.0));
    }

    #[test]
    fn does_not_fire_cross_below_after_crossover() {
        let mut detector = CrossDetector::new();
        detector.update("rule_0", 52.0, 50.0);
        assert!(detector.is_cross_below("rule_0", 48.0, 50.0));
        detector.update("rule_0", 48.0, 50.0);
        assert!(!detector.is_cross_below("rule_0", 45.0, 50.0));
    }

    #[test]
    fn independent_state_per_rule() {
        let mut detector = CrossDetector::new();
        detector.update("rule_0", 48.0, 50.0);
        // rule_1 has no previous values
        assert!(!detector.is_cross_above("rule_1", 52.0, 50.0));
    }
}
```

### PaperBroker tests (`paper.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_broker() -> PaperBroker {
        PaperBroker::new("NIFTY".to_string(), 100_000.0)
    }

    fn buy_order(qty: usize, price: f64) -> Order {
        Order { side: OrderSide::Buy, symbol: "NIFTY".to_string(), quantity: qty, price, .. Default::default() }
    }

    fn sell_order(qty: usize, price: f64) -> Order {
        Order { side: OrderSide::Sell, symbol: "NIFTY".to_string(), quantity: qty, price, .. Default::default() }
    }

    #[tokio::test]
    async fn buy_deducts_cash() {
        let mut broker = make_broker();
        broker.execute(buy_order(10, 500.0)).await.unwrap();
        assert_eq!(broker.get_state().cash, 95_000.0);
    }

    #[tokio::test]
    async fn buy_updates_position() {
        let mut broker = make_broker();
        broker.execute(buy_order(10, 500.0)).await.unwrap();
        let pos = broker.get_position("NIFTY").unwrap();
        assert_eq!(pos.quantity, 10);
        assert_eq!(pos.avg_entry_price, 500.0);
    }

    #[tokio::test]
    async fn two_buys_average_entry_price() {
        let mut broker = make_broker();
        broker.execute(buy_order(10, 500.0)).await.unwrap();
        broker.execute(buy_order(10, 600.0)).await.unwrap();
        let pos = broker.get_position("NIFTY").unwrap();
        assert_eq!(pos.avg_entry_price, 550.0);  // (10*500 + 10*600) / 20
    }

    #[tokio::test]
    async fn sell_credits_cash_and_calculates_pnl() {
        let mut broker = make_broker();
        broker.execute(buy_order(10, 500.0)).await.unwrap();
        broker.execute(sell_order(10, 600.0)).await.unwrap();
        let state = broker.get_state();
        // cash: 100000 - 5000 + 6000 = 101000
        assert_eq!(state.cash, 101_000.0);
        assert_eq!(state.total_realized_pnl, 1_000.0);
    }

    #[tokio::test]
    async fn buy_fails_insufficient_funds() {
        let mut broker = make_broker();
        // try to buy more than we have
        let result = broker.execute(buy_order(1000, 200.0)).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind,
            ExecutionErrorKind::InsufficientFunds
        ));
    }

    #[tokio::test]
    async fn sell_all_with_no_position_fails() {
        let mut broker = make_broker();
        let order = Order { side: OrderSide::SellAll, symbol: "NIFTY".to_string(), quantity: 0, price: 500.0, .. Default::default() };
        let result = broker.execute(order).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind,
            ExecutionErrorKind::InsufficientPosition
        ));
    }

    #[tokio::test]
    async fn reset_restores_initial_state() {
        let mut broker = make_broker();
        broker.execute(buy_order(10, 500.0)).await.unwrap();
        broker.reset();
        let state = broker.get_state();
        assert_eq!(state.cash, 100_000.0);
        assert!(state.positions.is_empty());
        assert!(state.trade_history.is_empty());
    }
}
```

---

## Layer 2 — Parser Manual Verification Test

Add this test to `parser.rs`. It prints the AST for visual inspection during development.
It is not an assertion test — it is a debug/print test.

```rust
#[test]
fn print_ast_for_inspection() {
    let src = "WHEN rsi(14) < 30\nBUY 5";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let node = Parser::new(tokens).parse().unwrap();
    println!("{:#?}", node);
    // run with: cargo test print_ast_for_inspection -- --nocapture
}

#[test]
fn print_parse_error_for_inspection() {
    let src = "WHEN rsi(14) <\nBUY 5";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    println!("{:#?}", result);
    assert!(result.is_err());
}
```

Run these with:
```bash
cargo test print_ast -- --nocapture
```

---

## Layer 3 — Engine Integration Tests (`engine.rs`)

These tests wire the full pipeline: parser → validator → engine → paper broker.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::dsl::{lexer::Lexer, parser::Parser, validator::AstValidator};
    use crate::strategy::execution::paper::PaperBroker;
    use chrono::Utc;

    /// Build a minimal candle. Only close price matters for most tests.
    fn candle(close: f64) -> Candle {
        Candle {
            open: close,
            high: close,
            low: close,
            close,
            volume: 1000.0,
            timestamp: Utc::now(),
            .. Default::default()
        }
    }

    fn make_engine(src: &str, initial_cash: f64) -> StrategyEngine {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let node = Parser::new(tokens).parse().unwrap();
        let errors = AstValidator::validate(&node);
        assert!(errors.is_empty(), "Validation failed: {:?}", errors);
        let broker = Arc::new(PaperBroker::new("TEST".to_string(), initial_cash));
        let instance = StrategyInstance {
            id: "test-strategy".to_string(),
            strategy: Arc::new(node),
            symbol: "TEST".to_string(),
            timeframe: Timeframe::Minutes(5),
            status: StrategyStatus::Running,
            execution_target: broker,
        };
        StrategyEngine::new(instance)
    }

    // -----------------------------------------------------------------------
    // Test 3a: Simple price comparison with trigger state
    // Strategy: WHEN close > 105 / BUY 1
    // Candles: 100, 101, 102, 103, 104, 105, 106, 107, 108
    // Expected: exactly one BUY on candle where close first exceeds 105 (i.e. 106)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn fires_exactly_once_on_condition_trigger() {
        let src = "WHEN close > 105\nBUY 1";
        let mut engine = make_engine(src, 100_000.0);

        let closes = vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0];
        let candles: Vec<Candle> = closes.iter().map(|&c| candle(c)).collect();

        let mut total_trades = 0;

        for i in 1..=candles.len() {
            let slice = &candles[..i];
            let logs = engine.on_candle(slice).await;
            let trades: Vec<_> = logs.iter().filter(|e| matches!(e.kind, LogEntryKind::OrderExecuted { .. })).collect();
            total_trades += trades.len();
        }

        assert_eq!(total_trades, 1, "Expected exactly 1 trade, got {}", total_trades);
    }

    // -----------------------------------------------------------------------
    // Test 3b: The "idiot test"
    // Strategy: WHEN close > 0 / BUY 1
    // Every candle satisfies this condition.
    // Expected: exactly ONE trade total (false → true fires once; true → true never fires again)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn idiot_test_fires_only_once() {
        let src = "WHEN close > 0\nBUY 1";
        let mut engine = make_engine(src, 100_000.0);

        let candles: Vec<Candle> = (1..=20).map(|i| candle(i as f64)).collect();
        let mut total_trades = 0;

        for i in 1..=candles.len() {
            let slice = &candles[..i];
            let logs = engine.on_candle(slice).await;
            total_trades += logs.iter().filter(|e| matches!(e.kind, LogEntryKind::OrderExecuted { .. })).count();
        }

        assert_eq!(total_trades, 1, "Idiot test failed: expected 1, got {}", total_trades);
    }

    // -----------------------------------------------------------------------
    // Test 3c: Condition resets and fires again
    // Strategy: WHEN close > 105 / BUY 1
    // Candles go above 105, then back below, then above again
    // Expected: 2 trades total
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn fires_again_after_condition_resets() {
        let src = "WHEN close > 105\nBUY 1";
        let mut engine = make_engine(src, 100_000.0);

        let closes = vec![100.0, 106.0, 100.0, 106.0];
        let candles: Vec<Candle> = closes.iter().map(|&c| candle(c)).collect();
        let mut total_trades = 0;

        for i in 1..=candles.len() {
            let slice = &candles[..i];
            let logs = engine.on_candle(slice).await;
            total_trades += logs.iter().filter(|e| matches!(e.kind, LogEntryKind::OrderExecuted { .. })).count();
        }

        assert_eq!(total_trades, 2);
    }

    // -----------------------------------------------------------------------
    // Test 4: Crossover logic
    // Strategy: WHEN cross_above(ma(2), ma(5)) / BUY 1
    //
    // You need enough candles for MA(5) to have a value, then engineer a cross.
    // Use candles designed so that:
    // - ma(2) starts below ma(5)
    // - on one specific candle, ma(2) crosses above ma(5)
    // - it stays above afterward
    //
    // Expected: exactly 1 BUY
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn cross_above_fires_exactly_once() {
        let src = "WHEN cross_above(ma(2), ma(5))\nBUY 1";
        let mut engine = make_engine(src, 100_000.0);

        // Descending then sharp spike upward forces a MA(2) cross above MA(5)
        let closes = vec![50.0, 49.0, 48.0, 47.0, 46.0, 45.0, 90.0, 92.0, 93.0, 94.0];
        let candles: Vec<Candle> = closes.iter().map(|&c| candle(c)).collect();
        let mut total_trades = 0;

        for i in 1..=candles.len() {
            let slice = &candles[..i];
            let logs = engine.on_candle(slice).await;
            total_trades += logs.iter().filter(|e| matches!(e.kind, LogEntryKind::OrderExecuted { .. })).count();
        }

        assert_eq!(total_trades, 1, "Cross above fired {} times, expected 1", total_trades);
    }
}
```

---

## Layer 4 — Backtest Integration Test

Create `src-tauri/src/strategy/tests/backtest_integration.rs`:

```rust
// src-tauri/src/strategy/tests/backtest_integration.rs

#[cfg(test)]
mod tests {
    use crate::strategy::{
        dsl::{lexer::Lexer, parser::Parser, ast::StrategyNode},
        execution::paper::{PaperBroker, PaperTrade},
        logging::log::LogEntry,
        BacktestResult,
    };
    use crate::commands::strategy::run_backtest_internal;
    use crate::data::Candle;
    use chrono::Utc;

    fn candle(close: f64) -> Candle {
        Candle { open: close, high: close, low: close, close, volume: 1000.0, timestamp: Utc::now(), .. Default::default() }
    }

    fn parse_strategy(src: &str) -> StrategyNode {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    // -----------------------------------------------------------------------
    // Backtest 1: Known output test
    // strategy: WHEN close > 105 / BUY 1
    // candles: 100 → 108
    // expected: 1 trade at close=106, PnL = 0 (no sell)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn backtest_known_output() {
        let strategy = parse_strategy("WHEN close > 105\nBUY 1");
        let candles: Vec<Candle> = (100..=108).map(|i| candle(i as f64)).collect();

        let result = run_backtest_internal(strategy, "TEST".to_string(), candles, 100_000.0).await.unwrap();

        assert_eq!(result.trade_history.len(), 1);
        assert_eq!(result.total_candles_processed, 9);
    }

    // -----------------------------------------------------------------------
    // Backtest 2: Determinism test
    // Run the exact same backtest twice. Results must be identical.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn backtest_is_deterministic() {
        let src = "WHEN close > 105\nBUY 1\n\nWHEN close < 102\nSELL ALL";
        let candles: Vec<Candle> = (98..=110).map(|i| candle(i as f64)).collect();

        let strategy1 = parse_strategy(src);
        let strategy2 = parse_strategy(src);

        let result1 = run_backtest_internal(strategy1, "TEST".to_string(), candles.clone(), 100_000.0).await.unwrap();
        let result2 = run_backtest_internal(strategy2, "TEST".to_string(), candles.clone(), 100_000.0).await.unwrap();

        assert_eq!(result1.trade_history.len(), result2.trade_history.len());
        assert_eq!(result1.total_realized_pnl, result2.total_realized_pnl);
        assert_eq!(result1.final_cash, result2.final_cash);

        for (t1, t2) in result1.trade_history.iter().zip(result2.trade_history.iter()) {
            assert_eq!(t1.price, t2.price);
            assert_eq!(t1.quantity, t2.quantity);
            assert_eq!(t1.side, t2.side);
        }
    }

    // -----------------------------------------------------------------------
    // Backtest 3: Buy then sell cycle — verify PnL
    // candles: 100, 110, 90
    // strategy: BUY when close > 105, SELL ALL when close < 95
    // expected: buy at 110, sell at 90, PnL = (90 - 110) * 1 = -20
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn backtest_buy_sell_pnl_is_correct() {
        let src = "WHEN close > 105\nBUY 1\n\nWHEN close < 95\nSELL ALL";
        let strategy = parse_strategy(src);
        let candles = vec![candle(100.0), candle(110.0), candle(90.0)];

        let result = run_backtest_internal(strategy, "TEST".to_string(), candles, 100_000.0).await.unwrap();

        assert_eq!(result.trade_history.len(), 2);
        assert_eq!(result.total_realized_pnl, -20.0);
    }
}
```

---

## How to Run

```bash
# Run all unit tests
cargo test

# Run only lexer tests
cargo test strategy::dsl::lexer::tests

# Run only engine tests
cargo test strategy::runtime::engine::tests

# Run backtest integration tests
cargo test strategy::tests::backtest_integration

# Run with output (for the print_ast tests)
cargo test print_ast -- --nocapture

# Run everything and show all output
cargo test -- --nocapture
```

---

## What Passing Looks Like

```
test strategy::dsl::lexer::tests::tokenizes_simple_strategy ... ok
test strategy::dsl::lexer::tests::tokenizes_sell_all ... ok
test strategy::dsl::lexer::tests::skips_comments ... ok
test strategy::dsl::lexer::tests::skips_blank_lines ... ok
test strategy::dsl::lexer::tests::distinguishes_lte_from_lt ... ok
test strategy::dsl::lexer::tests::integer_vs_float ... ok
test strategy::dsl::lexer::tests::unknown_character_produces_lex_error ... ok
test strategy::dsl::lexer::tests::cross_above_tokenizes_correctly ... ok

test strategy::dsl::parser::tests::parses_simple_rsi_rule ... ok
test strategy::dsl::parser::tests::assigns_sequential_rule_ids ... ok
test strategy::dsl::parser::tests::sell_all_is_sell_all_not_sell_quantity ... ok
test strategy::dsl::parser::tests::parses_cross_above ... ok
test strategy::dsl::parser::tests::parses_and_condition ... ok
test strategy::dsl::parser::tests::parses_not_condition ... ok
test strategy::dsl::parser::tests::parse_error_on_missing_action ... ok
test strategy::dsl::parser::tests::parse_error_on_incomplete_comparison ... ok
test strategy::dsl::parser::tests::deterministic_ids_across_reparses ... ok

test strategy::dsl::validator::tests::valid_strategy_has_no_errors ... ok
test strategy::dsl::validator::tests::rejects_empty_strategy ... ok
test strategy::dsl::validator::tests::rejects_zero_period ... ok
test strategy::dsl::validator::tests::rejects_zero_quantity ... ok
test strategy::dsl::validator::tests::rejects_cross_with_literal_operand ... ok
test strategy::dsl::validator::tests::collects_all_errors_not_just_first ... ok

test strategy::runtime::trigger_state::tests::fires_on_false_to_true ... ok
test strategy::runtime::trigger_state::tests::does_not_fire_on_true_to_true ... ok
test strategy::runtime::trigger_state::tests::does_not_fire_on_true_to_false ... ok
test strategy::runtime::trigger_state::tests::does_not_fire_on_false_to_false ... ok
test strategy::runtime::trigger_state::tests::resets_and_fires_again_after_false ... ok
test strategy::runtime::trigger_state::tests::independent_state_per_rule ... ok

test strategy::runtime::cross::tests::returns_false_with_no_previous_values ... ok
test strategy::runtime::cross::tests::detects_cross_above_on_exact_candle ... ok
test strategy::runtime::cross::tests::does_not_fire_cross_above_after_crossover ... ok
test strategy::runtime::cross::tests::detects_cross_below_on_exact_candle ... ok
test strategy::runtime::cross::tests::does_not_fire_cross_below_after_crossover ... ok
test strategy::runtime::cross::tests::independent_state_per_rule ... ok

test strategy::execution::paper::tests::buy_deducts_cash ... ok
test strategy::execution::paper::tests::buy_updates_position ... ok
test strategy::execution::paper::tests::two_buys_average_entry_price ... ok
test strategy::execution::paper::tests::sell_credits_cash_and_calculates_pnl ... ok
test strategy::execution::paper::tests::buy_fails_insufficient_funds ... ok
test strategy::execution::paper::tests::sell_all_with_no_position_fails ... ok
test strategy::execution::paper::tests::reset_restores_initial_state ... ok

test strategy::runtime::engine::tests::fires_exactly_once_on_condition_trigger ... ok
test strategy::runtime::engine::tests::idiot_test_fires_only_once ... ok
test strategy::runtime::engine::tests::fires_again_after_condition_resets ... ok
test strategy::runtime::engine::tests::cross_above_fires_exactly_once ... ok

test strategy::tests::backtest_integration::backtest_known_output ... ok
test strategy::tests::backtest_integration::backtest_is_deterministic ... ok
test strategy::tests::backtest_integration::backtest_buy_sell_pnl_is_correct ... ok
```

If any test fails, fix it before proceeding to live candle data.
If the determinism test fails, that is a red-alert bug — stop everything and fix it first.

---

## What You Should NOT Do

- Do not connect to Dhan API in any of these tests.
- Do not read from disk or network in any of these tests.
- Do not modify implementation files to make tests pass in a way that breaks other tests.
- Do not skip the idiot test. It catches the most common trigger-state bug.
- Do not skip the determinism test. It catches the most dangerous class of trading engine bug.
