-- Enable TradingView by default and make it the primary live-price source.
-- priority 0 ranks it above Yahoo (1), so latest quotes for the kinds it
-- supports (stocks, crypto, forex) come from TradingView; historical backfill
-- and unsupported kinds still fall through to Yahoo (TradingView returns
-- NotSupported there, so the registry continues the chain).
UPDATE market_data_providers
SET enabled = TRUE, priority = 0
WHERE id = 'TRADINGVIEW';
