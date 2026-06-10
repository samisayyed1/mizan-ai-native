/**
 * Mizan landing — root layout.
 *
 * Font registration via @fontsource packages exposed as the
 * `--font-sans / --font-serif / --font-mono` CSS variables that
 * `globals.css` consumes. Dark mode locked at the root.
 */
import type { Metadata } from "next";
import "@fontsource-variable/inter";
import "@fontsource/merriweather/400.css";
import "@fontsource/merriweather/400-italic.css";
import "@fontsource/merriweather/700.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "@fontsource/ibm-plex-mono/600.css";
import "@fontsource/ibm-plex-mono/700.css";
import "./globals.css";

const SITE_URL = (
  process.env.NEXT_PUBLIC_SITE_URL ?? "https://getmizan.net"
).replace(/\/$/, "");

// Index only on the production deploy. Netlify sets CONTEXT=production
// for the live site and deploy-preview / branch-deploy for previews, so
// staging never leaks into search.
const IS_PROD =
  process.env.CONTEXT === "production" ||
  // local prod builds with no CONTEXT default to indexable (the deployed
  // prod build always has CONTEXT set, so this only affects local).
  (!process.env.CONTEXT && process.env.NODE_ENV === "production");

const TITLE = "Mizan — Know your net worth, down to the cent.";
const DESCRIPTION =
  "Bank accounts, stocks, ETFs, crypto, property, gold, pensions, sukuks — every account you own, reconciled in one audit-grade ledger. Accurate to the cent, current to the second. AI-native, launching August 2026.";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: TITLE,
    template: "%s · Mizan",
  },
  description: DESCRIPTION,
  applicationName: "Mizan",
  authors: [{ name: "Mizan", url: SITE_URL }],
  creator: "Mizan",
  publisher: "Mizan",
  category: "finance",
  generator: "Next.js",
  alternates: { canonical: "/" },
  keywords: [
    "Mizan",
    "AI wealth platform",
    "personal wealth management",
    "net worth tracker",
    "AI financial assistant",
    "multi-currency portfolio tracker",
    "crypto and equities tracker",
    "audit-grade ledger",
    "halal wealth tracker",
    "Zakat calculator",
    "sukuk tracker",
  ],
  openGraph: {
    type: "website",
    url: SITE_URL,
    title: TITLE,
    description:
      "Every account you own, reconciled in one audit-grade ledger. AI-native, end-to-end encrypted. Launching August 2026.",
    siteName: "Mizan",
    locale: "en_US",
    images: [
      {
        url: "/api/og",
        width: 1200,
        height: 630,
        alt: "Mizan — Know your net worth, down to the cent.",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: TITLE,
    description:
      "Every account you own, reconciled in one audit-grade ledger. Accurate to the cent, current to the second. Launching August 2026.",
    creator: "@getmizan",
    site: "@getmizan",
    images: ["/api/og"],
  },
  icons: {
    icon: [
      { url: "/logo-mark.svg", type: "image/svg+xml" },
      { url: "/app-icon-192.png", sizes: "192x192", type: "image/png" },
      { url: "/app-icon-512.png", sizes: "512x512", type: "image/png" },
    ],
    apple: [{ url: "/apple-touch-icon.png", sizes: "180x180" }],
  },
  manifest: "/site.webmanifest",
  appleWebApp: {
    capable: true,
    title: "Mizan",
    statusBarStyle: "black-translucent",
  },
  formatDetection: { telephone: false },
  robots: {
    index: IS_PROD,
    follow: IS_PROD,
    googleBot: {
      index: IS_PROD,
      follow: IS_PROD,
      "max-image-preview": "large",
      "max-snippet": -1,
      "max-video-preview": -1,
    },
  },
};

const orgJsonLd = {
  "@context": "https://schema.org",
  "@type": "Organization",
  "@id": `${SITE_URL}/#organization`,
  name: "Mizan",
  url: SITE_URL,
  logo: `${SITE_URL}/logo-mark.svg`,
  image: `${SITE_URL}/api/og`,
  description:
    "Mizan is an AI-native personal wealth platform that brings every account you own into one audit-grade ledger.",
  foundingLocation: { "@type": "Place", name: "Singapore" },
  email: "info@getmizan.net",
  contactPoint: {
    "@type": "ContactPoint",
    email: "info@getmizan.net",
    contactType: "customer support",
  },
} as const;

const websiteJsonLd = {
  "@context": "https://schema.org",
  "@type": "WebSite",
  "@id": `${SITE_URL}/#website`,
  name: "Mizan",
  url: SITE_URL,
  inLanguage: "en",
  publisher: { "@id": `${SITE_URL}/#organization` },
} as const;

const productJsonLd = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "Mizan",
  applicationCategory: "FinanceApplication",
  operatingSystem: "macOS, Windows, Linux, iOS, Android",
  url: SITE_URL,
  inLanguage: "en",
  description: DESCRIPTION,
  screenshot: `${SITE_URL}/api/og`,
  datePublished: "2026-08-01",
  featureList: [
    "Net worth tracking across 12 asset classes",
    "Multi-currency, multi-jurisdiction portfolio",
    "AI-native insights and suggestions",
    "Immutable hash-chained audit ledger",
    "Zakat calculation across all four schools",
    "AES-256-GCM encryption",
  ],
} as const;

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <head>
        <meta name="theme-color" content="hsl(0 0% 4.5%)" />
        <meta name="color-scheme" content="dark" />
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{
            __html: JSON.stringify([orgJsonLd, websiteJsonLd, productJsonLd]),
          }}
        />
        {process.env.NEXT_PUBLIC_PLAUSIBLE_DOMAIN ? (
          <script
            defer
            data-domain={process.env.NEXT_PUBLIC_PLAUSIBLE_DOMAIN}
            src="https://plausible.io/js/script.js"
          />
        ) : null}
      </head>
      <body
        style={{
          // Wire the @fontsource imports into the CSS variables that
          // globals.css consumes.
          ["--font-sans" as string]: "'Inter Variable', 'Inter', system-ui",
          ["--font-serif" as string]:
            "'Merriweather', ui-serif, Georgia, serif",
          ["--font-mono" as string]: "'IBM Plex Mono', ui-monospace, monospace",
        }}
      >
        <a href="#main" className="skip-link">
          Skip to content
        </a>
        {children}
      </body>
    </html>
  );
}
