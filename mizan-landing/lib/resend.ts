import "server-only";

import { Resend } from "resend";

/**
 * Resend client — server-only. If `RESEND_API_KEY` isn't set we
 * return a stub that no-ops, so local dev + preview branches don't
 * crash and the API route can record `emailSent: false` cleanly.
 */
interface ResendLike {
  emails: {
    send: (...args: Parameters<Resend["emails"]["send"]>) => Promise<
      | { data: { id: string }; error: null }
      | { data: null; error: { message: string; name: string } }
      | { skipped: true }
    >;
  };
  /**
   * Add a signup to the Resend Audience (the mailing list we broadcast
   * to at launch). No-ops if RESEND_API_KEY or RESEND_AUDIENCE_ID is
   * unset — the Supabase row is still the source of truth, so a failure
   * here never blocks a signup.
   */
  addContact: (email: string) => Promise<{ ok: boolean; skipped?: boolean }>;
}

let cached: ResendLike | undefined;

export function getResend(): ResendLike {
  if (cached) return cached;

  const key = process.env.RESEND_API_KEY;
  const audienceId = process.env.RESEND_AUDIENCE_ID;

  if (!key) {
    cached = {
      emails: {
        send: async () => ({ skipped: true as const }),
      },
      addContact: async () => ({ ok: false, skipped: true }),
    };
    return cached;
  }

  const r = new Resend(key);
  cached = {
    emails: {
      send: async (payload) => {
        const { data, error } = await r.emails.send(payload);
        if (error) return { data: null, error };
        return { data: { id: data!.id }, error: null };
      },
    },
    addContact: async (email) => {
      if (!audienceId) return { ok: false, skipped: true };
      try {
        const { error } = await r.contacts.create({
          email,
          audienceId,
          unsubscribed: false,
        });
        return { ok: !error };
      } catch {
        return { ok: false };
      }
    },
  };
  return cached;
}

export const RESEND_FROM =
  process.env.RESEND_FROM ?? "Mizan <info@getmizan.net>";
