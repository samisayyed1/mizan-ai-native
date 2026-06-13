-- Mizan waitlist — Supabase schema.
--
-- Run via: supabase db push (locally) or supabase migration up (remote).
--
-- Design notes:
--   - id      : opaque UUID, never surfaced to the user.
--   - ref_code: 8-char base32-ish (we drop +/= from base64) so it's
--               URL-safe + memorable; defaulted server-side so the
--               API route doesn't have to generate it.
--   - referred_by: attribution only — never bumps anyone's position
--                  (research-driven: position-race mechanics are dead
--                  in 2026 premium SaaS).
--   - position: bigserial, monotonically increasing, surfaced to the
--               user as "founding member #N".
--   - email   : unique. The API returns 409 + the existing position
--               on duplicate so the form is idempotent.
--
-- RLS: enabled with NO anon policy. All writes go through the
-- service-role client in the route handler. The Supabase anon key is
-- never used by this table.

create extension if not exists pgcrypto;

create table if not exists public.waitlist (
  id           uuid primary key default gen_random_uuid(),
  email        text not null unique,
  country      text not null,
  pain_point   text,
  ref_code     text not null unique
                 default replace(replace(substr(encode(gen_random_bytes(6),'base64'), 1, 8), '+', 'A'), '/', 'B'),
  referred_by  text references public.waitlist(ref_code) on delete set null,
  position     bigserial,
  created_at   timestamptz not null default now()
);

create index if not exists waitlist_ref_code_idx on public.waitlist (ref_code);
create index if not exists waitlist_referred_by_idx on public.waitlist (referred_by);

alter table public.waitlist enable row level security;
-- No anon policies on purpose. Writes via service-role only.
