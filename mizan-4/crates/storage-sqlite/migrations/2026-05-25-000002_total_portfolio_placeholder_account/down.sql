-- Removes the TOTAL placeholder account. Note: this is destructive —
-- any TOTAL holdings_snapshots will also be deleted via ON DELETE CASCADE.

DELETE FROM accounts WHERE id = 'TOTAL';
