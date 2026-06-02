-- Mizan Connect — surface payment failures + trial end deadlines on
-- the subscriptions row so the desktop can render remediation UI
-- without waiting for the eventual customer.subscription.updated.
--
-- last_payment_failure_at:
--   Stamped on `invoice.payment_failed`. The subscription stays
--   "active" until Stripe's smart-retry policy ages it out to
--   "past_due"; mirroring this marker locally lets the desktop nudge
--   the user to update their card the moment the first retry fails
--   instead of after Stripe gives up days later.
--
-- trial_end_at:
--   Populated on `customer.subscription.trial_will_end` (Stripe's
--   7-day heads-up). The desktop renders a "Your trial ends on
--   YYYY-MM-DD" banner so the conversion isn't a surprise charge.
--
-- Forward-only. Both columns nullable + NULL by default; existing
-- subscriptions get NULL until the next webhook for that sub.

ALTER TABLE subscriptions
    ADD COLUMN IF NOT EXISTS last_payment_failure_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS trial_end_at TIMESTAMPTZ;
