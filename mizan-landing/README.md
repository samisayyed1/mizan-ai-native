# Mizan landing — getmizan.net

Standalone Next.js 14 marketing surface + waitlist for [Mizan](../mizan-4), the AI-native personal wealth platform for the Muslim affluent. Launches August 2026.

This package is intentionally isolated from `mizan-4/` (desktop) and `mizan-connect/` (backend): no shared `package.json`, no shared workspace. Brand tokens are ported by hand from the desktop's `globals.css` so this bundle stays small.

## Stack

| Layer | Choice | Why |
|---|---|---|
| Framework | Next.js 14 App Router | RSC by default, smallest JS payload |
| Styling | Tailwind 3.4 + design tokens | Matches desktop's token discipline |
| Motion | Framer Motion 11 | Respects `prefers-reduced-motion` via `useReducedMotion()` |
| Icons | `lucide-react` | Tree-shaken per icon |
| DB | Supabase (Postgres) | Service-role only; RLS-locked table |
| Email | Resend (+ React Email) | Stub when key missing |
| Rate-limit | Upstash Redis sliding window | In-memory fallback for dev |
| OG | `@vercel/og` edge | One image, cached 24h |
| Analytics | Plausible | Header script + server-side custom events |
| Hosting | Vercel (LHR region) | Lowest hop from London founder |

## Prereqs

- Node 20+
- pnpm 9+

## Local dev

```bash
pnpm install
cp .env.example .env.local   # fill in keys; all are optional locally
pnpm dev
open http://localhost:3000
```

With no env vars set:
- Supabase calls fail loudly when you submit the form (expected — set `SUPABASE_URL` + `SUPABASE_SERVICE_ROLE` to test end-to-end).
- Resend silently no-ops.
- Rate limit runs in-process.
- Plausible script + server event skip.

## Database

```bash
pnpm dlx supabase db push          # local docker
# or:
pnpm dlx supabase migration up     # remote, after `supabase link`
```

The migration is at [`supabase/migrations/0001_waitlist.sql`](supabase/migrations/0001_waitlist.sql). RLS is enabled with NO anon policies — all writes flow through the service-role client in the route handler.

## Tests

```bash
pnpm playwright install --with-deps chromium    # one-time
pnpm test:visual                                 # full-page snapshots × 4 viewports
pnpm test:visual --update-snapshots              # accept new baselines
pnpm lhci autorun                                # Lighthouse CI (95+ on every category)
```

## Deploy

First time:
```bash
pnpm dlx vercel link
pnpm dlx vercel env add SUPABASE_URL                 # repeat per env var below
```

Required production env vars:
- `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE`
- `RESEND_API_KEY`, `RESEND_FROM`
- `UPSTASH_REDIS_REST_URL`, `UPSTASH_REDIS_REST_TOKEN`
- `NEXT_PUBLIC_SITE_URL=https://getmizan.net`
- `NEXT_PUBLIC_PLAUSIBLE_DOMAIN=getmizan.net`

Then:
```bash
pnpm dlx vercel --prod
```

## DNS

Point `getmizan.net` `A` (apex) and `www` `CNAME` at the Vercel record shown after `vercel domains add`. Vercel auto-provisions TLS via Let's Encrypt; verify HSTS preload after the first successful issuance.

## Conventions

- **No leaderboards, no position-bumping.** Research-backed: gamified waitlists read spammy in 2026 fintech. Attribution captured for analytics only.
- **Gold is restrained.** Used for CTAs, focus rings, wordmark tittle, and exactly one section background tint (Zakat). Everywhere else: depth ladder + foreground.
- **Wordmark is Merriweather serif bold**, mirroring the desktop sidebar.
- **All four madhāhib** are surfaced equally — no school is "default."
- **August 2026** launch date is hardcoded in copy. Update Hero badge, FoundingOffer body, email template, and OG image together when the date shifts.

## Out of scope

- Desktop app (`../mizan-4`)
- Backend / sync workers (`../mizan-connect`)
- Anything in `mizan-connect/.env.fly` — that file lives at `~/Documents/mizan-ai-native/.env.fly`, never commit
