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
}

let cached: ResendLike | undefined;

export function getResend(): ResendLike {
  if (cached) return cached;

  const key = process.env.RESEND_API_KEY;
  if (!key) {
    cached = {
      emails: {
        send: async () => ({ skipped: true as const }),
      },
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
  };
  return cached;
}

export const RESEND_FROM =
  process.env.RESEND_FROM ?? "Sami <sami@getmizan.net>";
