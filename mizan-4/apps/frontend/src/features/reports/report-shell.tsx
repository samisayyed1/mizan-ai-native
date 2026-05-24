import type { ReactNode } from "react";

/**
 * A4 page dimensions at 96 DPI: 794 × 1123 px.
 * The renderer (`useReportRenderer`) renders this off-screen, then paginates
 * via html2canvas + jsPDF. Keep the visual block widths within the body width
 * (the body padding subtracts margins).
 */
export const A4_WIDTH_PX = 794;
export const A4_HEIGHT_PX = 1123;

const PAGE_HORIZONTAL_PADDING_PX = 48;
const PAGE_VERTICAL_PADDING_PX = 56;

export interface ReportShellProps {
  /** Bold title shown at the top of every page (e.g. "Income report"). */
  title: string;
  /** Subtitle (e.g. period "Jan – Dec 2026"). Optional. */
  subtitle?: string;
  /**
   * URL of an org / team branding logo. Drawn top-left at ~32 px tall. If
   * omitted, the Mizan wordmark is used.
   */
  logoUrl?: string;
  /**
   * Optional accent color (CSS color) applied to dividers and the footer
   * border. Used by white-label reports (M5.4).
   */
  accentColor?: string;
  /**
   * Disclaimer at the bottom of every page. Defaults to the standard
   * "Not financial advice" line; reports may override per locale.
   */
  disclaimer?: string;
  /** Body content — typically a stack of `<ReportSection>`s. */
  children: ReactNode;
  /**
   * Page label shown bottom-right. The renderer overrides this with the
   * paginated page number; while authoring you can leave it blank.
   */
  pageLabel?: string;
}

/**
 * Opinionated A4 layout used by every Pro report. Width is fixed so
 * html2canvas captures consistently across machines; height grows freely
 * and the renderer slices it into A4 pages.
 *
 * Do NOT use a CSS framework class that depends on viewport units here —
 * the off-screen render container is detached from the viewport.
 */
export function ReportShell({
  title,
  subtitle,
  logoUrl,
  accentColor = "#3f6212",
  disclaimer = "This report summarizes your own data. Mizan does not provide investment advice.",
  children,
  pageLabel,
}: ReportShellProps) {
  return (
    <div
      className="mizan-report"
      data-mizan-report=""
      style={{
        width: `${A4_WIDTH_PX}px`,
        minHeight: `${A4_HEIGHT_PX}px`,
        padding: `${PAGE_VERTICAL_PADDING_PX}px ${PAGE_HORIZONTAL_PADDING_PX}px`,
        background: "#ffffff",
        color: "#100f0f",
        fontFamily: '"Inter", "Helvetica Neue", Arial, sans-serif',
        fontSize: "12px",
        lineHeight: 1.5,
        boxSizing: "border-box",
      }}
    >
      <header
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "flex-end",
          paddingBottom: "16px",
          borderBottom: `2px solid ${accentColor}`,
          marginBottom: "24px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          {logoUrl ? (
            <img
              src={logoUrl}
              alt=""
              style={{ height: "32px", width: "auto", objectFit: "contain" }}
            />
          ) : (
            <span
              style={{
                fontSize: "20px",
                fontWeight: 600,
                letterSpacing: "-0.01em",
                color: accentColor,
              }}
            >
              Mizan
            </span>
          )}
          <div style={{ display: "flex", flexDirection: "column" }}>
            <span style={{ fontSize: "16px", fontWeight: 600 }}>{title}</span>
            {subtitle && <span style={{ fontSize: "12px", color: "#6f6e69" }}>{subtitle}</span>}
          </div>
        </div>
        <span style={{ fontSize: "11px", color: "#6f6e69" }}>
          Generated{" "}
          {new Date().toLocaleDateString(undefined, {
            year: "numeric",
            month: "long",
            day: "numeric",
          })}
        </span>
      </header>

      <main style={{ display: "flex", flexDirection: "column", gap: "20px" }}>{children}</main>

      <footer
        style={{
          marginTop: "32px",
          paddingTop: "12px",
          borderTop: `1px solid ${accentColor}`,
          display: "flex",
          justifyContent: "space-between",
          fontSize: "10px",
          color: "#6f6e69",
        }}
      >
        <span>{disclaimer}</span>
        {pageLabel && <span data-mizan-report-page-label="">{pageLabel}</span>}
      </footer>
    </div>
  );
}
