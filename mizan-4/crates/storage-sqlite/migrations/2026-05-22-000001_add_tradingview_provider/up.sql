-- Add the TradingView screener as a fallback market-data provider.
-- Disabled by default: it is an unofficial, ToS-restricted endpoint, so the
-- user must explicitly enable it. NULL logo renders the generic provider icon
-- (no missing-image). Priority 50 keeps it below all other providers, so it is
-- only consulted when higher-priority providers (e.g. Yahoo) have no quote.
INSERT OR IGNORE INTO market_data_providers
    (id, name, description, url, priority, enabled, logo_filename, last_synced_at, last_sync_status, last_sync_error)
VALUES
    ('TRADINGVIEW', 'TradingView (Screener)', 'Unofficial fallback that fetches the latest price from TradingView''s public screener when the primary provider has no quote. Latest price only; covers stocks, crypto and forex. Disabled by default; enable at your own risk under TradingView''s terms of service.', 'https://www.tradingview.com/', 50, FALSE, NULL, NULL, NULL, NULL);
