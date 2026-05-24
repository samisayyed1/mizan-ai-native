---
name: stripe-test-mode
description: Use when testing Stripe Checkout, Billing Portal, or webhook handling in test mode. Includes the canonical `stripe trigger` commands and the test-card matrix.
---

# Stripe test mode end-to-end

Stripe powers the Free → Silver → Gold tier upgrade. Test mode keys only
until v3 Phase N validation passes. See v3 §Stripe End-to-End Contract.

## Credential request

```
STRIPE_SECRET_KEY=sk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...
STRIPE_PRICE_SILVER_MONTHLY=price_...
STRIPE_PRICE_SILVER_YEARLY=price_...
STRIPE_PRICE_GOLD_MONTHLY=price_...
STRIPE_PRICE_GOLD_YEARLY=price_...
STRIPE_CUSTOMER_PORTAL_RETURN_URL=
STRIPE_CHECKOUT_SUCCESS_URL=
STRIPE_CHECKOUT_CANCEL_URL=
```

If any missing, **stop** and ask. Stripe is unforgiving — wrong
price IDs silently sell the wrong tier.

## Local webhook forwarding

Install the Stripe CLI, log in once, then in a separate terminal:

```bash
stripe listen --forward-to localhost:8080/v1/stripe/webhook
# Note the printed whsec_... and set STRIPE_WEBHOOK_SECRET to it
```

## Smoke flow

1. **Checkout session** — desktop calls:

   ```
   POST /v1/billing/checkout-session
   body: { price_id, success_url, cancel_url }
   → { url }   # 200 only when auth + price_id valid
   ```

2. **Test card** — complete checkout with `4242 4242 4242 4242`,
   any future expiry, any CVC, any postal.

3. **Webhook delivery** — `stripe listen` should print the events:
   - `checkout.session.completed`
   - `customer.subscription.created`
   - `customer.subscription.updated`
     Webhook handler must be **signature-verified** and **idempotent**
     (`event_id` UNIQUE in `stripe_events`).

4. **Entitlement refresh** — desktop calls `GET /v1/me` after return.
   Expect `tier=Silver` (or Gold), `subscription.status=active`,
   `capabilities.managedAi=true`.

5. **Trigger non-happy paths**:

   ```bash
   stripe trigger customer.subscription.deleted    # cancellation
   stripe trigger invoice.payment_failed           # past_due
   stripe trigger customer.subscription.updated    # downgrade
   ```

   Each must end with `/v1/me` reflecting the new state correctly
   within 5 s.

6. **Customer Portal** — `POST /v1/billing/portal` returns the URL.
   User can cancel; webhook fires; `/v1/me` returns to Free.

## What to assert

- Checkout session creates `subscriptions` row keyed by `team_id`
  (post Phase G+ schema).
- Webhook handler is idempotent — replaying the same event yields 0
  state change.
- Desktop entitlements update within 5 s of webhook delivery.
- Free → Silver unlocks managed AI + CSV import + alt assets.
- Silver → Gold unlocks Plaid + SnapTrade + Zakat.
- Cancellation downgrades to Free; entitlement cache flushes.
- `/v1/me` reflects truth — never trust the Checkout success-url query
  params alone.

## Never

- Never put `sk_live_*` in `.env.example`.
- Never log full webhook payloads (Stripe events contain payment-method
  hints).
- Never trust the `tier` claim from anywhere but `/v1/me`.
- Never flip to live mode before v3 Phase N validation passes.

## Production gate (P1+)

Locked until MVP validation. Then: separate live price IDs, separate
webhook endpoint, billing portal live, failed-payment retry policy,
cancellation grace period, downgrade UX, entitlement cache invalidation,
receipts / invoices, tax / VAT setup if required.
