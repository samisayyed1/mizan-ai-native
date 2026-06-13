import { z } from "zod";

/**
 * Canonical jurisdictions Mizan supports at launch + post-launch.
 * Mirrors the launch + Group-B set documented in
 * `docs/SHIPPING-CHECKLIST.md`.
 */
export const COUNTRIES = [
  "UK",
  "USA",
  "UAE",
  "KSA",
  "Malaysia",
  "Singapore",
  "Canada",
  "Australia",
  "Other",
] as const;
export type Country = (typeof COUNTRIES)[number];

export const waitlistSchema = z.object({
  email: z.string().email().max(254).transform((s) => s.toLowerCase().trim()),
  country: z.enum(COUNTRIES),
  painPoint: z.string().max(280).optional().nullable().transform((s) => s?.trim() || undefined),
  ref: z.string().length(8).optional().nullable().transform((s) => s || undefined),
});

export type WaitlistInput = z.infer<typeof waitlistSchema>;

export interface WaitlistResponse {
  position: number;
  refCode: string;
  alreadyRegistered?: boolean;
}
