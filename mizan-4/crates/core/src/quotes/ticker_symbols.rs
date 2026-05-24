//! Curated market symbols streamed in the dashboard ticker conveyor alongside
//! the user's own holdings.
//!
//! Each entry pairs the symbol the resolver/provider expects with a friendly
//! label. The exact symbol forms (e.g. `SPX` vs `^GSPC`, `CL` vs `USOIL`) must
//! be confirmed against a live `resolve_symbol_quote` run; unresolved symbols
//! degrade gracefully to a price-less chip rather than failing the ticker.

use crate::assets::InstrumentType;

/// A curated index/commodity to show in the ticker.
pub struct CuratedTickerSymbol {
    /// Symbol passed to the resolver (TradingView-style root where relevant).
    pub symbol: &'static str,
    /// Friendly label shown in the chip.
    pub label: &'static str,
    pub instrument_type: InstrumentType,
}

/// Major indices + commodities + crypto shown after the user's holdings.
pub const CURATED_TICKER_SYMBOLS: &[CuratedTickerSymbol] = &[
    CuratedTickerSymbol {
        symbol: "SPX",
        label: "S&P 500",
        instrument_type: InstrumentType::Equity,
    },
    CuratedTickerSymbol {
        symbol: "IXIC",
        label: "Nasdaq",
        instrument_type: InstrumentType::Equity,
    },
    CuratedTickerSymbol {
        symbol: "DJI",
        label: "Dow Jones",
        instrument_type: InstrumentType::Equity,
    },
    CuratedTickerSymbol {
        symbol: "BTCUSD",
        label: "Bitcoin",
        instrument_type: InstrumentType::Crypto,
    },
    CuratedTickerSymbol {
        symbol: "XAUUSD",
        label: "Gold",
        instrument_type: InstrumentType::Metal,
    },
    CuratedTickerSymbol {
        symbol: "CL",
        label: "Crude Oil",
        instrument_type: InstrumentType::Futures,
    },
];
