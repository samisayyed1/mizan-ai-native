import "server-only";

import { Ratelimit } from "@upstash/ratelimit";
import { Redis } from "@upstash/redis";

/**
 * Sliding-window rate limit: 5 attempts per IP per 10 minutes.
 *
 * When the Upstash env vars aren't set (local dev, preview branches
 * without the secret) we fall back to an in-memory limiter that's
 * good enough for one process — the API route still degrades safely.
 */
const url = process.env.UPSTASH_REDIS_REST_URL;
const token = process.env.UPSTASH_REDIS_REST_TOKEN;

interface RateLimiter {
  limit: (key: string) => Promise<{ success: boolean; remaining: number }>;
}

let cached: RateLimiter | undefined;

export function getRateLimit(): RateLimiter {
  if (cached) return cached;

  if (url && token) {
    const redis = new Redis({ url, token });
    const rl = new Ratelimit({
      redis,
      limiter: Ratelimit.slidingWindow(5, "10 m"),
      analytics: true,
      prefix: "mizan-landing:waitlist",
    });
    cached = {
      limit: async (key) => {
        const { success, remaining } = await rl.limit(key);
        return { success, remaining };
      },
    };
    return cached;
  }

  // In-memory fallback for dev. Per-process, lost on restart — fine.
  const hits = new Map<string, { count: number; reset: number }>();
  cached = {
    limit: async (key) => {
      const now = Date.now();
      const windowMs = 10 * 60 * 1000;
      const existing = hits.get(key);
      if (!existing || existing.reset < now) {
        hits.set(key, { count: 1, reset: now + windowMs });
        return { success: true, remaining: 4 };
      }
      existing.count += 1;
      const success = existing.count <= 5;
      return { success, remaining: Math.max(0, 5 - existing.count) };
    },
  };
  return cached;
}

/**
 * Convenience wrapper for route handlers: returns
 * `{ success, retryAfter }`. `retryAfter` is the full window in
 * seconds — the underlying limiter doesn't expose a precise
 * countdown, so we surface the worst case as a "try again later" hint.
 */
export async function rateLimit(
  key: string,
): Promise<{ success: boolean; retryAfter: number }> {
  const { success } = await getRateLimit().limit(key);
  return { success, retryAfter: success ? 0 : 600 };
}
