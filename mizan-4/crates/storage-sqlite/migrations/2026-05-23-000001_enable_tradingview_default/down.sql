UPDATE market_data_providers
SET enabled = FALSE, priority = 50
WHERE id = 'TRADINGVIEW';
