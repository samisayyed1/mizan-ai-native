import type { ReactNode } from "react";

export interface ReportSectionProps {
  /** Section heading (e.g. "Year-to-date dividends"). */
  title: string;
  /** Optional supporting line under the title (e.g. asset count, date range). */
  caption?: string;
  /**
   * Optional callout shown to the right of the heading — useful for the
   * dominant metric of the section (e.g. "$12,450.32").
   */
  metric?: ReactNode;
  /** Section body — tables, charts, paragraphs. */
  children: ReactNode;
}

/**
 * Section primitive used inside `<ReportShell>`. Avoids CSS framework
 * classes so it renders identically inside the detached off-screen
 * container used by `useReportRenderer`.
 */
export function ReportSection({ title, caption, metric, children }: ReportSectionProps) {
  return (
    <section
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "8px",
        breakInside: "avoid",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "baseline",
          gap: "16px",
        }}
      >
        <div style={{ display: "flex", flexDirection: "column" }}>
          <h2 style={{ fontSize: "14px", fontWeight: 600, margin: 0 }}>{title}</h2>
          {caption && <span style={{ fontSize: "11px", color: "#6f6e69" }}>{caption}</span>}
        </div>
        {metric && (
          <span style={{ fontSize: "18px", fontWeight: 600, whiteSpace: "nowrap" }}>{metric}</span>
        )}
      </div>
      <div style={{ fontSize: "12px", color: "#100f0f" }}>{children}</div>
    </section>
  );
}
