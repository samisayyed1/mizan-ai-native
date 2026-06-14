-- ============================================================================
-- INSURANCE_PRODUCT + PROVIDENT_FUND root categories for the
-- `instrument_type` taxonomy.
-- ============================================================================
--
-- Why
-- ---
-- The dashboard's 12 asset-class tiles include `insurance` and
-- `provident-funds` (see
-- `apps/frontend/src/components/asset-class-panels/taxonomy.ts:25-150`),
-- but the taxonomy seeded by 2026-01-01-000002 only ships keys for
-- equities / bonds / funds / private / real-asset / cash / digital /
-- forex. ULIPs and CPF-balance-style provident balances had nowhere
-- to land — the classifier would fall through to `brokerage-accounts`
-- (its INVESTMENT-kind fallback).
--
-- These two categories close that gap. The frontend classifier learns
-- the matching keys in the same PR
-- (`taxonomy.ts::classifyHolding` adds two `if` branches), so the
-- Uncle Feroz seed's 4 ULIP policies and 1 CPF balance now route to
-- their intended tiles instead of `brokerage-accounts`.
--
-- Colour stops chosen to extend the existing palette without clashing
-- with the gold-ladder used by panels. Sort orders 11 + 12 place the
-- new roots after the existing 10 root categories.

INSERT INTO taxonomy_categories (
    id,
    taxonomy_id,
    parent_id,
    name,
    key,
    color,
    sort_order
) VALUES
  ('INSURANCE_PRODUCT', 'instrument_type', NULL, 'Insurance',       'INSURANCE_PRODUCT', '#7e63a8', 11),
  ('PROVIDENT_FUND',    'instrument_type', NULL, 'Provident Funds', 'PROVIDENT_FUND',    '#5e7f9c', 12);
