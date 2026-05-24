import { ReportSection, ReportShell } from "..";

export interface IncomeReportData {
  /** Period label, e.g. "Jan – Dec 2026" or "YTD 2026". */
  periodLabel: string;
  /** User's base currency (e.g. "USD"). */
  currency: string;
  /** Total income across the period. */
  totalIncome: number;
  /** Average monthly income across the reported months. */
  monthlyAverage: number;
  /** Year-over-year growth as a fraction (e.g. 0.05 for +5%). */
  yoyGrowth: number | null;
  /** Income totals by type ("DIVIDEND", "INTEREST", "RENTAL", etc.). */
  byType: Record<string, number>;
  /** Top-N income-producing assets, already sorted desc by total. */
  topAssets: { symbol: string; name?: string; total: number; type?: string }[];
  /** Monthly breakdown (key = ISO month, value = total in base currency). */
  byMonth: Record<string, number>;
}

const fmtCurrency = (amount: number, currency: string) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(amount);

const fmtPct = (pct: number) => `${pct > 0 ? "+" : ""}${(pct * 100).toFixed(1)}%`;

/**
 * Pro income report. Renders into a `<ReportShell>` so the PDF
 * renderer can capture it off-screen.
 *
 * The caller is responsible for assembling `IncomeReportData` from the
 * existing `useIncomeSummary` hook + alternative-asset rentals.
 */
export function IncomeReport({ data }: { data: IncomeReportData }) {
  const sortedMonths = Object.entries(data.byMonth).sort(([a], [b]) => a.localeCompare(b));
  const monthMax = Math.max(0, ...Object.values(data.byMonth));

  return (
    <ReportShell title="Income report" subtitle={data.periodLabel}>
      <ReportSection
        title="Total income"
        caption={`${data.periodLabel} · ${data.currency}`}
        metric={fmtCurrency(data.totalIncome, data.currency)}
      >
        <p>
          Monthly average: {fmtCurrency(data.monthlyAverage, data.currency)}
          {data.yoyGrowth != null && (
            <>
              {" · "}
              YoY change:{" "}
              <span style={{ color: data.yoyGrowth >= 0 ? "#3f6212" : "#a13f1c" }}>
                {fmtPct(data.yoyGrowth)}
              </span>
            </>
          )}
          .
        </p>
      </ReportSection>

      <ReportSection title="Income by type">
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ borderBottom: "1px solid #cecdc3" }}>
              <th style={{ textAlign: "left", padding: "6px 4px" }}>Type</th>
              <th style={{ textAlign: "right", padding: "6px 4px" }}>Amount</th>
              <th style={{ textAlign: "right", padding: "6px 4px" }}>Share</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(data.byType)
              .sort(([, a], [, b]) => b - a)
              .map(([type, amount]) => (
                <tr key={type} style={{ borderBottom: "1px solid #eeece2" }}>
                  <td style={{ padding: "6px 4px" }}>{type}</td>
                  <td style={{ textAlign: "right", padding: "6px 4px" }}>
                    {fmtCurrency(amount, data.currency)}
                  </td>
                  <td style={{ textAlign: "right", padding: "6px 4px", color: "#6f6e69" }}>
                    {data.totalIncome > 0
                      ? `${((amount / data.totalIncome) * 100).toFixed(1)}%`
                      : "—"}
                  </td>
                </tr>
              ))}
          </tbody>
        </table>
      </ReportSection>

      <ReportSection
        title="Top income-producing assets"
        caption={`${data.topAssets.length} positions ranked by total income`}
      >
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ borderBottom: "1px solid #cecdc3" }}>
              <th style={{ textAlign: "left", padding: "6px 4px" }}>Symbol</th>
              <th style={{ textAlign: "left", padding: "6px 4px" }}>Type</th>
              <th style={{ textAlign: "right", padding: "6px 4px" }}>Total income</th>
            </tr>
          </thead>
          <tbody>
            {data.topAssets.map((asset) => (
              <tr key={asset.symbol} style={{ borderBottom: "1px solid #eeece2" }}>
                <td style={{ padding: "6px 4px", fontWeight: 500 }}>
                  {asset.symbol}
                  {asset.name && (
                    <span style={{ color: "#6f6e69", fontWeight: 400 }}> · {asset.name}</span>
                  )}
                </td>
                <td style={{ padding: "6px 4px", color: "#6f6e69" }}>{asset.type ?? "—"}</td>
                <td style={{ textAlign: "right", padding: "6px 4px" }}>
                  {fmtCurrency(asset.total, data.currency)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </ReportSection>

      {sortedMonths.length > 0 && (
        <ReportSection title="Monthly trend" caption={`${sortedMonths.length} months`}>
          <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
            {sortedMonths.map(([month, amount]) => {
              const width = monthMax > 0 ? Math.round((amount / monthMax) * 100) : 0;
              return (
                <div
                  key={month}
                  style={{
                    display: "grid",
                    gridTemplateColumns: "80px 1fr 100px",
                    alignItems: "center",
                    gap: "8px",
                    fontSize: "11px",
                  }}
                >
                  <span style={{ color: "#6f6e69" }}>{month}</span>
                  <div
                    style={{
                      height: "10px",
                      background: "#eeece2",
                      borderRadius: "2px",
                      overflow: "hidden",
                    }}
                  >
                    <div
                      style={{
                        width: `${width}%`,
                        height: "100%",
                        background: "#3f6212",
                      }}
                    />
                  </div>
                  <span style={{ textAlign: "right" }}>{fmtCurrency(amount, data.currency)}</span>
                </div>
              );
            })}
          </div>
        </ReportSection>
      )}
    </ReportShell>
  );
}
