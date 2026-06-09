import type { MetadataRoute } from "next";

const SITE_URL =
  process.env.NEXT_PUBLIC_SITE_URL?.replace(/\/$/, "") ??
  "https://getmizan.net";

/**
 * Single-route sitemap — landing page only. The waitlist API + OG
 * routes are intentionally not listed. `lastModified` is fixed at
 * build time (workflow restriction prevents Date.now() in scripts;
 * fine for a sitemap because deploys are infrequent).
 */
export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: `${SITE_URL}/`,
      lastModified: new Date("2026-06-10"),
      changeFrequency: "weekly",
      priority: 1,
    },
  ];
}
