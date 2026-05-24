import { ReportSection, ReportShell } from "..";

export interface RentalProperty {
  /** Display label (e.g. "47 Maple St #2"). */
  label: string;
  /** Tenant name (optional). */
  tenantName?: string;
  /** Monthly rent in the report currency. */
  monthlyRent: number;
  /** Lease start date (ISO). */
  rentalStartDate?: string;
  /** Lease end date (ISO), if set. */
  rentalEndDate?: string;
  /** Optional notes (e.g. property manager, utilities). */
  notes?: string;
}

export interface RentalReportData {
  currency: string;
  /** Period the report covers (e.g. "2026 YTD"). */
  periodLabel: string;
  /** All rented properties; one section rendered per property. */
  properties: RentalProperty[];
}

const fmtCurrency = (amount: number, currency: string) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(amount);

/**
 * Pro rental income report. One section per rented property + a totals
 * section at the top.
 */
export function RentalReport({ data }: { data: RentalReportData }) {
  const annualByProperty = data.properties.map((p) => p.monthlyRent * 12);
  const totalAnnual = annualByProperty.reduce((sum, v) => sum + v, 0);
  const totalMonthly = data.properties.reduce((sum, p) => sum + p.monthlyRent, 0);

  return (
    <ReportShell title="Rental income" subtitle={data.periodLabel}>
      <ReportSection
        title="Projected annual income"
        caption={`${data.properties.length} rented ${data.properties.length === 1 ? "property" : "properties"} · ${data.currency}`}
        metric={fmtCurrency(totalAnnual, data.currency)}
      >
        <p>
          Combined monthly rent: {fmtCurrency(totalMonthly, data.currency)}. The annual projection
          assumes uninterrupted occupancy and current rates.
        </p>
      </ReportSection>

      {data.properties.length === 0 ? (
        <ReportSection title="No rented properties">
          <p>
            None of your properties are currently set up as rentals. Add a rental amount on a
            property to include it in this report.
          </p>
        </ReportSection>
      ) : (
        data.properties.map((property, idx) => (
          <ReportSection
            key={idx}
            title={property.label}
            metric={`${fmtCurrency(property.monthlyRent, data.currency)} / mo`}
            caption={property.tenantName ? `Tenant: ${property.tenantName}` : undefined}
          >
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 1fr 1fr",
                gap: "12px",
              }}
            >
              <div>
                <div style={{ fontSize: "11px", color: "#6f6e69" }}>Annual projection</div>
                <div style={{ fontWeight: 600 }}>
                  {fmtCurrency(property.monthlyRent * 12, data.currency)}
                </div>
              </div>
              <div>
                <div style={{ fontSize: "11px", color: "#6f6e69" }}>Lease start</div>
                <div>{property.rentalStartDate ?? "—"}</div>
              </div>
              <div>
                <div style={{ fontSize: "11px", color: "#6f6e69" }}>Lease end</div>
                <div>{property.rentalEndDate ?? "Open-ended"}</div>
              </div>
            </div>
            {property.notes && (
              <p
                style={{
                  marginTop: "8px",
                  fontSize: "11px",
                  color: "#6f6e69",
                }}
              >
                {property.notes}
              </p>
            )}
          </ReportSection>
        ))
      )}
    </ReportShell>
  );
}
