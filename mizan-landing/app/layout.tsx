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

const SITE_URL =
  process.env.NEXT_PUBLIC_SITE_URL ?? "https://getmizan.net";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: "Mizan — Know your net worth, down to the cent.",
  description:
    "Bank accounts, stocks, ETFs, crypto, property, gold, pensions, sukuks — every account you own, reconciled in one audit-grade ledger. Accurate to the cent, current to the second. AI-native. Launching August 2026.",
  applicationName: "Mizan",
  authors: [{ name: "Mizan" }],
  generator: "Next.js",
  keywords: [
    "Mizan",
    "personal wealth management",
    "net worth tracker",
    "AI financial assistant",
    "multi-currency portfolio",
    "crypto and equities tracker",
    "audit-grade ledger",
    "Zakat calculator",
  ],
  openGraph: {
    type: "website",
    url: SITE_URL,
    title: "Mizan — Know your net worth, down to the cent.",
    description:
      "Bank accounts, stocks, ETFs, crypto, property, gold, pensions, sukuks — every account you own, reconciled in one audit-grade ledger. AI-native, end-to-end encrypted.",
    siteName: "Mizan",
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
    title: "Mizan — Know your net worth, down to the cent.",
    description:
      "Every account you own, reconciled in one audit-grade ledger. Accurate to the cent, current to the second. Launching August 2026.",
    creator: "@getmizan",
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
  robots: {
    index: process.env.VERCEL_ENV === "production",
    follow: process.env.VERCEL_ENV === "production",
  },
};

const orgJsonLd = {
  "@context": "https://schema.org",
  "@type": "Organization",
  name: "Mizan",
  url: SITE_URL,
  logo: `${SITE_URL}/logo-mark.svg`,
  foundingLocation: "Singapore",
} as const;

const productJsonLd = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "Mizan",
  applicationCategory: "FinanceApplication",
  operatingSystem: "macOS, Windows, Linux, iOS, Android",
  url: SITE_URL,
  offers: {
    "@type": "Offer",
    priceCurrency: "GBP",
    availabilityStarts: "2026-08-01",
  },
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
            __html: JSON.stringify([orgJsonLd, productJsonLd]),
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
