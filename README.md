# AlgoMLN

**A fast, local-first algorithmic trading platform built in Rust.**

AlgoMLN is a desktop trading application focused on reliability, determinism, and extensibility.

Built by an algo trader, for algo traders.

---

## Philosophy

Most trading platforms are built UI-first.

AlgoMLN is built engine-first.

```text
Data
↓
Indicators
↓
Strategy Engine
↓
Backtesting
↓
Execution
↓
UI
```

The goal is simple:

* Fast startup
* No cloud dependency
* No mandatory account system
* Paper trading first
* Deterministic strategy execution
* Extensible architecture

---

# Current Status

## Completed

### Phase 1 — Data Layer

* Broker abstraction architecture
* Historical OHLCV retrieval
* Market data models
* Dhan integration
* Streaming market data support
* Internal data pipelines

### Phase 2 — Indicator Engine

Implemented indicators:

* SMA
* EMA
* RSI
* ATR
* VWAP
* Bollinger Bands

Indicators are implemented as pure Rust functions and are fully testable.

---

### Phase 2.5 — Strategy Language

A custom trading DSL was designed and implemented.

Example:

```algo
WHEN rsi(14) < 30
BUY 1

WHEN rsi(14) > 70
SELL ALL
```

and

```algo
WHEN cross_above(ema(20), ema(50))
BUY 1

WHEN cross_below(ema(20), ema(50))
SELL ALL
```

---

### Phase 2.6 — Compiler Pipeline

The strategy language is compiled using:

```text
Source
↓
Lexer
↓
Parser
↓
AST
↓
Validator
↓
Runtime
```

Implemented:

* Lexer
* Parser
* AST
* Validation system
* Error reporting

---

### Phase 2.7 — Strategy Runtime

Implemented:

* Rule evaluation engine
* Trigger state tracking
* Cross detection system
* Indicator provider abstraction
* Strategy execution pipeline

---

### Phase 2.8 — Paper Trading Engine

Implemented:

* PaperBroker
* Position tracking
* Cash tracking
* Trade history
* Realized PnL calculation
* Execution target abstraction

---

### Phase 2.9 — Backtesting

Implemented:

* Historical replay engine
* Candle-by-candle execution
* Deterministic execution model
* Integration testing

---

# Testing Status

Current test results:

```text
88 passed
0 failed
1 ignored
```

---

# Architecture

## Broker Abstraction

AlgoMLN separates execution from broker implementations.

```text
Strategy Engine
        ↓
ExecutionTarget
        ↓
┌─────────────┬─────────────┬─────────────┐
│ PaperBroker │ DhanBroker  │ UpstoxBroker│
└─────────────┴─────────────┴─────────────┘
```

The strategy engine never knows which broker is executing orders.

---

## Strategy Compiler

Strategies are compiled into an Abstract Syntax Tree.

```algo
WHEN rsi(14) < 30
BUY 1
```

becomes:

```text
Rule
├── Condition
│   └── RSI(14) < 30
└── Action
    └── BUY 1
```

The runtime executes the AST rather than interpreting raw text.

---

## Trigger State System

One of the biggest problems in trading engines:

```algo
WHEN close > 0
BUY 1
```

Without protection:

```text
BUY
BUY
BUY
BUY
BUY
...
```

on every candle.

AlgoMLN uses a Trigger State system that only fires on:

```text
false → true
```

transitions.

This means:

```algo
WHEN rsi(14) < 30
BUY 1
```

executes once when RSI enters oversold territory, not every candle afterward.

---

## Cross Detection

Crossovers are surprisingly tricky.

AlgoMLN stores previous indicator values and detects transitions:

```text
EMA20 <= EMA50
        ↓
EMA20 > EMA50
```

Only the crossover candle triggers.

Remaining above does not generate additional signals.

---

## Deterministic Execution

The same strategy executed on the same candles must always produce the same result.

This is a core design goal.

The engine avoids non-deterministic evaluation paths and is designed so that:

```text
Strategy
+
Market Data
=
Identical Results
```

every run.

This property is critical for reliable backtesting.

---

## Engine Architecture

```text
Candles
    ↓
Indicator Provider
    ↓
Condition Evaluation
    ↓
Trigger State
    ↓
Order Builder
    ↓
Execution Target
    ↓
PaperBroker / Live Broker
```

---

# Why A Custom DSL?

Most retail trading platforms either:

* force visual builders
* expose raw Python
* require external tools

AlgoMLN takes a different approach.

Strategies are written in a small domain-specific language:

```algo
WHEN cross_above(ema(20), ema(50))
BUY 10
```

Simple enough for beginners.

Structured enough for reliable compilation.

---

# Roadmap

## Phase 3 — Charts & Core UI

Planned:

* Lightweight Charts integration
* Symbol switching
* Indicator overlays
* Timeframe selection
* Support/Resistance overlays

---

## Phase 4 — Trading Tools

Planned:

* Option Chain
* Open Interest analysis
* Payoff diagrams
* Screeners

---

## Phase 5 — Visual Strategy Builder

Planned:

```text
Drag & Drop Blocks
        ↓
Generated DSL
        ↓
AST
        ↓
Strategy Engine
```

The visual builder and text strategies will share the exact same runtime.

---

## Phase 6 — Advanced Strategy Authoring

Planned:

* More indicators
* Position sizing
* Risk controls
* Stop losses
* Take profits
* Multi-condition strategies

---

## Phase 7 — Live Trading

Planned:

* Live broker execution
* Risk validation
* Confirmation workflows
* Immutable trade logs
* User-defined safety limits

---

# Long-Term Vision

AlgoMLN aims to become a complete algorithmic trading platform where:

```text
Research
↓
Strategy Design
↓
Backtest
↓
Paper Trade
↓
Live Trade
```

all happen inside one application using the same execution engine.

No hidden execution paths.

No separate runtimes.

No discrepancies between backtesting and live execution.

---

# Current Milestone

The strategy engine, paper broker, DSL, compiler pipeline, and backtesting infrastructure are operational and passing test coverage.

The next major milestone is the first charting and visualization layer.
