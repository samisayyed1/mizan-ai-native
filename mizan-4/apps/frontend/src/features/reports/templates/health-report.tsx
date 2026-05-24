import { ReportSection, ReportShell } from "..";

export interface HealthReportData {
  currency: string;
  /** Composite 0-100 score; null when portfolio is empty. */
  score: number | null;
  /** Per-driver breakdown — `id` matches `HealthDriver::id` from Rust. */
  drivers: {
    id: string;
    label: string;
    score: number;
    metric: number;
    note: string;
  }[];
  /** The lowest-scoring driver; used for the headline callout. */
  worstDriver: { id: string; label: string; score: number; note: string } | null;
  /** Plain-text disclaimers from the Rust math layer. */
  notes: string[];
}

const scoreBand = (score: number): { label: string; color: string } => {
  if (score >= 80) return { label: "Strong", color: "#3f6212" };
  if (score >= 60) return { label: "Good", color: "#5e7a14" };
  if (score >= 40) return { label: "Mixed", color: "#a16d1c" };
  return { label: "Needs attention", color: "#a13f1c" };
};

export function HealthReport({ data }: { data: HealthReportData }) {
  const composite = data.score ?? 0;
  const band = scoreBand(composite);

  return (
    <ReportShell title="Portfolio health" subtitle={`Composite score · ${data.currency}`}>
      <ReportSection
        title="Overall health"
        metric={
          <span style={{ color: band.color }}>
            {data.score == null ? "—" : composite.toFixed(0)}
            <span style={{ fontSize: "12px", color: "#6f6e69" }}> / 100</span>
          </span>
        }
      >
        {data.score == null ? (
          <p>Portfolio has no positions yet — add holdings to score it.</p>
        ) : (
          <>
            <p>
              Status: <span style={{ color: band.color, fontWeight: 600 }}>{band.label}</span>.
            </p>
            {data.worstDriver && (
              <p style={{ marginTop: "4px" }}>
                Weakest driver: <strong>{data.worstDriver.label}</strong> (
                {data.worstDriver.score.toFixed(0)}/100) — {data.worstDriver.note}
              </p>
            )}
          </>
        )}
      </ReportSection>

      <ReportSection
        title="Driver breakdown"
        caption="Each driver scored 0–100. Higher = healthier."
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          {data.drivers.map((d) => {
            const dBand = scoreBand(d.score);
            return (
              <div key={d.id}>
                <div
                  style={{
                    display: "grid",
                    gridTemplateColumns: "1fr 60px",
                    alignItems: "baseline",
                    gap: "8px",
                  }}
                >
                  <span style={{ fontWeight: 500 }}>{d.label}</span>
                  <span
                    style={{
                      textAlign: "right",
                      color: dBand.color,
                      fontWeight: 600,
                    }}
                  >
                    {d.score.toFixed(0)}
                  </span>
                </div>
                <div
                  style={{
                    height: "8px",
                    background: "#eeece2",
                    borderRadius: "2px",
                    overflow: "hidden",
                    marginTop: "4px",
                  }}
                >
                  <div
                    style={{
                      width: `${Math.max(0, Math.min(100, d.score))}%`,
                      height: "100%",
                      background: dBand.color,
                    }}
                  />
                </div>
                <p
                  style={{
                    margin: "4px 0 0",
                    fontSize: "11px",
                    color: "#6f6e69",
                  }}
                >
                  {d.note}
                </p>
              </div>
            );
          })}
        </div>
      </ReportSection>

      {data.notes.length > 0 && (
        <ReportSection title="Notes">
          <ul style={{ margin: 0, paddingLeft: "16px" }}>
            {data.notes.map((note, idx) => (
              <li key={idx} style={{ fontSize: "11px", color: "#6f6e69" }}>
                {note}
              </li>
            ))}
          </ul>
        </ReportSection>
      )}
    </ReportShell>
  );
}
