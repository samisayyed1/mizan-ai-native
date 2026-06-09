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
        {/* Wordmark */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            position: "relative",
            zIndex: 1,
          }}
        >
          <span
            style={{
              fontFamily: "Georgia, 'Times New Roman', serif",
              fontWeight: 700,
              fontSize: 40,
              color: GOLD_CREAM,
              letterSpacing: "-0.02em",
              display: "flex",
            }}
          >
            Mizan
          </span>
          <div
            style={{
              width: 10,
              height: 10,
              borderRadius: 9999,
              background: GOLD_PRIMARY,
              marginLeft: 6,
              marginTop: -22,
              display: "flex",
            }}
          />
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
            Coming August 2026
          </div>
          <div
            style={{
              fontFamily: "Georgia, 'Times New Roman', serif",
              fontWeight: 700,
              fontSize: 78,
              lineHeight: 1.05,
              color: GOLD_CREAM,
              letterSpacing: "-0.025em",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <span style={{ display: "flex" }}>Your wealth,</span>
            <span style={{ display: "flex", color: FOREGROUND }}>
              accounted for.
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
            12 asset classes · 9 jurisdictions · Zakat across all four schools
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
          <span style={{ display: "flex" }}>Founding members open</span>
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
