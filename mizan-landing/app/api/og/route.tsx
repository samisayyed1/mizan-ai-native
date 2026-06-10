/**
 * GET /api/og — dynamic OpenGraph image (1200×630).
 *
 * Pure CSS/JSX — no fetched fonts. @vercel/og's default fonts cover
 * Latin glyphs cleanly enough for our headline. Cached for 24h.
 */
import { ImageResponse } from "@vercel/og";

export const runtime = "edge";

const GOLD_CREAM = "#F5E6C8";
const GOLD_PRIMARY = "#D4A574";
const GOLD_DEEP = "#8B6F47";
const PAGE = "#0B0B0B";
const FOREGROUND = "#CBC9BC";

export async function GET(): Promise<ImageResponse> {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          background: PAGE,
          padding: 80,
          position: "relative",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        {/* Single subtle gold orb upper right */}
        <div
          style={{
            position: "absolute",
            top: -200,
            right: -200,
            width: 700,
            height: 700,
            borderRadius: 9999,
            background: `radial-gradient(circle, ${GOLD_PRIMARY} 0%, transparent 60%)`,
            opacity: 0.3,
            display: "flex",
          }}
        />
        {/* Wordmark — balance mark + "Mizan" */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 14,
            position: "relative",
            zIndex: 1,
          }}
        >
          {/* Mizan M-mark — rendered as an SVG so the brand reads
              identically on social previews and in the in-page wordmark. */}
          <svg
            width={56}
            height={56}
            viewBox="0 0 64 64"
            xmlns="http://www.w3.org/2000/svg"
            style={{ display: "flex" }}
          >
            <defs>
              <linearGradient id="og-g" x1="0" y1="0" x2="64" y2="64" gradientUnits="userSpaceOnUse">
                <stop offset="0%" stopColor={GOLD_CREAM} />
                <stop offset="55%" stopColor={GOLD_PRIMARY} />
                <stop offset="100%" stopColor={GOLD_DEEP} />
              </linearGradient>
            </defs>
            <rect x="1" y="1" width="62" height="62" rx="14" fill="#171717" stroke={GOLD_DEEP} strokeOpacity="0.35" strokeWidth="0.5" />
            <circle cx="32" cy="32" r="22" fill="none" stroke="url(#og-g)" strokeWidth="0.6" opacity="0.4" />
            <path d="M 18 46 L 18 18 L 24 18 L 32 32 L 40 18 L 46 18 L 46 46 L 41 46 L 41 27 L 34 39 L 30 39 L 23 27 L 23 46 Z" fill="url(#og-g)" />
          </svg>
          <span
            style={{
              fontFamily: "Georgia, 'Times New Roman', serif",
              fontWeight: 700,
              fontSize: 44,
              color: GOLD_CREAM,
              letterSpacing: "-0.02em",
              display: "flex",
            }}
          >
            Mizan
          </span>
        </div>

        {/* Headline + sub */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            position: "relative",
            zIndex: 1,
            maxWidth: 940,
          }}
        >
          <div
            style={{
              fontFamily: "ui-monospace, 'SF Mono', monospace",
              fontSize: 18,
              color: GOLD_PRIMARY,
              letterSpacing: "0.16em",
              textTransform: "uppercase",
              marginBottom: 24,
              display: "flex",
            }}
          >
            AI-Native Wealth Platform
          </div>
          <div
            style={{
              fontFamily: "Georgia, 'Times New Roman', serif",
              fontWeight: 700,
              fontSize: 78,
              lineHeight: 1.08,
              color: GOLD_CREAM,
              letterSpacing: "-0.025em",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <span style={{ display: "flex" }}>Know your net worth.</span>
            <span style={{ display: "flex", color: FOREGROUND }}>
              Down to the cent.
            </span>
          </div>
          <div
            style={{
              fontSize: 26,
              color: FOREGROUND,
              opacity: 0.75,
              marginTop: 28,
              lineHeight: 1.4,
              display: "flex",
            }}
          >
            Every account you own · one audit-grade ledger
          </div>
        </div>

        {/* Footer */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            position: "relative",
            zIndex: 1,
            color: GOLD_DEEP,
            fontSize: 18,
            fontFamily: "ui-monospace, 'SF Mono', monospace",
            letterSpacing: "0.05em",
            textTransform: "uppercase",
          }}
        >
          <span style={{ display: "flex" }}>getmizan.net</span>
          <span style={{ display: "flex" }}>Join the waitlist</span>
        </div>
      </div>
    ),
    {
      width: 1200,
      height: 630,
      headers: {
        "Cache-Control": "public, max-age=86400, immutable",
      },
    },
  );
}
