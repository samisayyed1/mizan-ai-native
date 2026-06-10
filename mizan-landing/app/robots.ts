import type { MetadataRoute } from "next";

const SITE_URL = (
  process.env.NEXT_PUBLIC_SITE_URL ?? "https://getmizan.net"
).replace(/\/$/, "");

/**
 * Allow indexing only on the production deploy. Netlify sets
 * CONTEXT=production for the live site and deploy-preview /
 * branch-deploy for previews, so staging never leaks into search.
 * Locally (no CONTEXT) we also disallow to be safe.
 */
export default function robots(): MetadataRoute.Robots {
  const isProd = process.env.CONTEXT === "production";
  if (!isProd) {
    return { rules: [{ userAgent: "*", disallow: "/" }] };
  }
  return {
    rules: [
      {
        userAgent: "*",
        allow: "/",
        // No private routes today; block the API + OG endpoints from
        // indexing since they aren't pages.
        disallow: ["/api/"],
      },
    ],
    sitemap: `${SITE_URL}/sitemap.xml`,
    host: SITE_URL,
  };
}
