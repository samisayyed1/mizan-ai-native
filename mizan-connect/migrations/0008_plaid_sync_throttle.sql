-- Mizan Connect — per-(user, item) Plaid sync cooldown.
-- Allows the sync handler to enforce a minimum interval between manual
-- /sync invocations from the same user against the same item, protecting
-- Plaid rate limits.

ALTER TABLE plaid_items
    ADD COLUMN IF NOT EXISTS last_sync_attempt_at TIMESTAMPTZ;
