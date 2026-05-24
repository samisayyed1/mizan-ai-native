import { ReportSection, ReportShell } from "..";

export interface PayoffReportData {
  /** Display name of the liability (e.g. "Wells Fargo mortgage"). */
  liabilityName: string;
  /** Currency for display. */
  currency: string;
  /** Monthly payment that will retire the loan. */
  monthlyPayment: number;
  /** Sum of all future payments under the schedule. */
  totalPaid: number;
  /** Sum of all future interest payments. */
  totalInterest: number;
  /** Date the loan finishes (ISO YYYY-MM-DD). */
  payoffDate: string;
  /** Installment count. */
  numberOfPayments: number;
  /** Per-installment schedule; render the first 12 + the last 6 inline. */
  schedule: {
    period: number;
    dueDate: string;
    payment: number;
    interest: number;
    principal: number;
    balanceAfter: number;
  }[];
  /** Disclaimers from the math layer. */
  notes: string[];
}

const fmtCurrency = (amount: number, currency: string) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: 2,
  }).format(amount);

/**
 * Pro liability-payoff report. Shows the loan summary, an inline
 * schedule (first year + the final stretch), and a remaining-balance
 * sparkline.
 */
export function PayoffReport({ data }: { data: PayoffReportData }) {
  const first12 = data.schedule.slice(0, 12);
  const last6 = data.schedule.length > 18 ? data.schedule.slice(-6) : data.schedule.slice(12);

  const balanceMax = Math.max(...data.schedule.map((i) => i.balanceAfter), 0);

  return (
    <ReportShell title="Liability payoff projection" subtitle={data.liabilityName}>
      <ReportSection
        title="Payoff summary"
        metric={fmtCurrency(data.monthlyPayment, data.currency)}
        caption="Monthly payment under the projected schedule"
      >
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr 1fr",
            gap: "12px",
          }}
        >
          <div>
            <div style={{ fontSize: "11px", color: "#6f6e69" }}>Payoff date</div>
            <div style={{ fontWeight: 600 }}>{data.payoffDate}</div>
            <div style={{ fontSize: "11px", color: "#6f6e69" }}>
              {data.numberOfPayments} payments
            </div>
          </div>
          <div>
            <div style={{ fontSize: "11px", color: "#6f6e69" }}>Total paid</div>
            <div style={{ fontWeight: 600 }}>{fmtCurrency(data.totalPaid, data.currency)}</div>
          </div>
          <div>
            <div style={{ fontSize: "11px", color: "#6f6e69" }}>Total interest</div>
            <div style={{ fontWeight: 600, color: "#a13f1c" }}>
              {fmtCurrency(data.totalInterest, data.currency)}
            </div>
          </div>
        </div>
      </ReportSection>

      <ReportSection
        title="Balance trajectory"
        caption="Remaining principal after each installment"
      >
        <div style={{ display: "flex", alignItems: "flex-end", gap: "1px", height: "60px" }}>
          {data.schedule.map((i) => {
            const h = balanceMax > 0 ? (i.balanceAfter / balanceMax) * 100 : 0;
            return (
              <div
                key={i.period}
                style={{
                  flex: 1,
                  height: `${h}%`,
                  background: "#3f6212",
                  opacity: 0.7,
                }}
                title={`#${i.period} · ${i.dueDate} · ${fmtCurrency(i.balanceAfter, data.currency)}`}
              />
            );
          })}
        </div>
      </ReportSection>

      <ReportSection title="First 12 months">
        <ScheduleTable rows={first12} currency={data.currency} />
      </ReportSection>

      {last6.length > 0 && data.schedule.length > 18 && (
        <ReportSection
          title="Final stretch"
          caption={`Last ${last6.length} installments before payoff`}
        >
          <ScheduleTable rows={last6} currency={data.currency} />
        </ReportSection>
      )}

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

function ScheduleTable({
  rows,
  currency,
}: {
  rows: PayoffReportData["schedule"];
  currency: string;
}) {
  return (
    <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "11px" }}>
      <thead>
        <tr style={{ borderBottom: "1px solid #cecdc3" }}>
          <th style={{ textAlign: "left", padding: "4px" }}>#</th>
          <th style={{ textAlign: "left", padding: "4px" }}>Due</th>
          <th style={{ textAlign: "right", padding: "4px" }}>Payment</th>
          <th style={{ textAlign: "right", padding: "4px" }}>Interest</th>
          <th style={{ textAlign: "right", padding: "4px" }}>Principal</th>
          <th style={{ textAlign: "right", padding: "4px" }}>Balance</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.period} style={{ borderBottom: "1px solid #eeece2" }}>
            <td style={{ padding: "4px", color: "#6f6e69" }}>{row.period}</td>
            <td style={{ padding: "4px" }}>{row.dueDate}</td>
            <td style={{ textAlign: "right", padding: "4px" }}>
              {fmtCurrency(row.payment, currency)}
            </td>
            <td style={{ textAlign: "right", padding: "4px", color: "#a13f1c" }}>
              {fmtCurrency(row.interest, currency)}
            </td>
            <td style={{ textAlign: "right", padding: "4px", color: "#3f6212" }}>
              {fmtCurrency(row.principal, currency)}
            </td>
            <td style={{ textAlign: "right", padding: "4px", fontWeight: 500 }}>
              {fmtCurrency(row.balanceAfter, currency)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
