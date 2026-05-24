import { describe, it, expect } from "vitest";
import { parseHoldingsSnapshots, type ParseOptions } from "./holdings-import-utils";
import { HoldingsFormat } from "../steps/holdings-mapping-step";

/**
 * Verifies the transaction-log-aware aggregation in
 * `parseHoldingsSnapshots`. The bug it fixes: when a Yahoo Portfolio
 * CSV (one row per BUY/SELL lot, with a Transaction Type column) was
 * routed through the Holdings Import path, the function silently
 * treated SELL rows as positive quantity and produced wildly wrong
 * positions. Verified end-to-end against uncle's actual 381-row file:
 * dashboard read $189K instead of the actual $260K market value.
 *
 * The shape of the test fixture below mirrors the real Yahoo
 * Portfolio CSV header (Transaction Type column with BUY/SELL,
 * Trade Date in YYYYMMDD compact form, Purchase Price as cost basis,
 * Quantity as lot size). Numbers chosen so the math is verifiable
 * by inspection.
 */

const DEFAULT_PARSE_OPTIONS: ParseOptions = {
  dateFormat: "ISO8601",
  decimalSeparator: ".",
  thousandsSeparator: ",",
  defaultCurrency: "USD",
};

const HOLDINGS_MAPPING_TX_LOG: Record<string, string> = {
  [HoldingsFormat.DATE]: "Trade Date",
  [HoldingsFormat.SYMBOL]: "Symbol",
  [HoldingsFormat.QUANTITY]: "Quantity",
  [HoldingsFormat.AVG_COST]: "Purchase Price",
  [HoldingsFormat.CURRENCY]: "Currency",
};

const TX_LOG_HEADERS = [
  "Symbol",
  "Current Price",
  "Trade Date",
  "Purchase Price",
  "Quantity",
  "Currency",
  "Transaction Type",
];

describe("parseHoldingsSnapshots — transaction-log mode (Yahoo Portfolio CSVs)", () => {
  it("nets BUY and SELL into a single position per symbol", () => {
    // AAPL: 50 bought, 10 sold → net 40
    // MSFT: 20 bought, no sells → net 20
    const rows = [
      ["AAPL", "294.80", "2025-08-15", "200.00", "50", "USD", "BUY"],
      ["AAPL", "294.80", "2026-05-10", "290.00", "10", "USD", "SELL"],
      ["MSFT", "407.77", "2025-12-01", "350.00", "20", "USD", "BUY"],
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out).toHaveLength(1);
    const snap = out[0];
    expect(snap.positions).toHaveLength(2);

    const aapl = snap.positions.find((p) => p.symbol === "AAPL")!;
    expect(parseFloat(aapl.quantity)).toBe(40); // 50 - 10
    // Avg cost only weighted across BUYs (no SELLs): 50 × 200 / 50 = 200
    expect(parseFloat(aapl.avgCost!)).toBeCloseTo(200, 4);

    const msft = snap.positions.find((p) => p.symbol === "MSFT")!;
    expect(parseFloat(msft.quantity)).toBe(20);
    expect(parseFloat(msft.avgCost!)).toBeCloseTo(350, 4);
  });

  it("weights cost basis correctly across multiple BUY lots", () => {
    // AAPL: 100 @ $150 + 50 @ $200 = 150 shares, weighted-avg cost
    //       (100×150 + 50×200) / 150 = (15000 + 10000) / 150 ≈ 166.67
    const rows = [
      ["AAPL", "294.80", "2024-01-01", "150.00", "100", "USD", "BUY"],
      ["AAPL", "294.80", "2024-06-01", "200.00", "50", "USD", "BUY"],
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out[0].positions).toHaveLength(1);
    const aapl = out[0].positions[0];
    expect(parseFloat(aapl.quantity)).toBe(150);
    expect(parseFloat(aapl.avgCost!)).toBeCloseTo(166.6667, 3);
  });

  it("drops symbols whose net quantity is zero (fully closed positions)", () => {
    // Bought 100 GOOG, sold all 100 → should NOT appear as a position.
    const rows = [
      ["GOOG", "150.00", "2024-01-01", "120.00", "100", "USD", "BUY"],
      ["GOOG", "150.00", "2025-12-15", "180.00", "100", "USD", "SELL"],
      ["AAPL", "294.80", "2025-01-01", "200.00", "10", "USD", "BUY"],
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out[0].positions).toHaveLength(1);
    expect(out[0].positions[0].symbol).toBe("AAPL");
  });

  it("drops watchlist rows (empty quantity / empty tx type)", () => {
    const rows = [
      ["AAPL", "294.80", "2025-08-15", "200.00", "50", "USD", "BUY"],
      // watchlist: no trade history at all
      ["WATCHED", "100.00", "", "", "", "USD", ""],
      // no tx type — drop (would otherwise be ambiguous)
      ["MSFT", "407.77", "2025-12-01", "350.00", "20", "USD", ""],
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out[0].positions).toHaveLength(1);
    expect(out[0].positions[0].symbol).toBe("AAPL");
  });

  it("drops zero-price BUY rows (junk / placeholder)", () => {
    // AWX.SI style row from uncle's real CSV — 50 shares @ $0 is a
    // "free shares" placeholder that pollutes the weighted-avg cost.
    const rows = [
      ["AAPL", "294.80", "2025-08-15", "200.00", "50", "USD", "BUY"],
      ["AAPL", "294.80", "2025-09-01", "0.00", "10", "USD", "BUY"], // junk
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out[0].positions).toHaveLength(1);
    const aapl = out[0].positions[0];
    expect(parseFloat(aapl.quantity)).toBe(50); // junk row dropped
    expect(parseFloat(aapl.avgCost!)).toBeCloseTo(200, 4);
  });

  it("produces a single snapshot dated today (not one per trade date)", () => {
    // The pre-fix bug created one snapshot per trade date — hundreds
    // of phantom snapshots. The fix collapses to a single snapshot
    // dated today regardless of how many distinct trade dates exist.
    const rows = [
      ["AAPL", "294.80", "2024-01-15", "200.00", "10", "USD", "BUY"],
      ["AAPL", "294.80", "2024-06-15", "210.00", "10", "USD", "BUY"],
      ["AAPL", "294.80", "2025-03-15", "220.00", "10", "USD", "BUY"],
      ["AAPL", "294.80", "2025-09-15", "230.00", "10", "USD", "BUY"],
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out).toHaveLength(1);
    // Today, ISO format. Just check shape, not exact value.
    expect(out[0].date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(out[0].positions).toHaveLength(1);
    expect(parseFloat(out[0].positions[0].quantity)).toBe(40);
  });

  it("uncle's full Yahoo Portfolio CSV — net position math sanity check", () => {
    // Faithful sample of uncle's actual file shape:
    //   AAPL: 50 bought + 25 bought + 20 bought = 95 BUYs; 10 sold; net = 85
    //   QBTS: 800 + 400 = 1200 BUYs, no sells; net = 1200
    //   GEV: 10 + 5 = 15 BUYs, 5 sold; net = 10
    //   FULLY_CLOSED: 50 bought + 50 sold; net = 0 (dropped)
    const rows = [
      ["AAPL", "294.80", "2025-01-01", "210.00", "50", "USD", "BUY"],
      ["AAPL", "294.80", "2025-06-01", "240.00", "25", "USD", "BUY"],
      ["AAPL", "294.80", "2026-01-15", "275.00", "20", "USD", "BUY"],
      ["AAPL", "294.80", "2026-05-08", "290.00", "10", "USD", "SELL"],
      ["QBTS", "21.72", "2025-11-01", "12.00", "800", "USD", "BUY"],
      ["QBTS", "21.72", "2026-02-15", "15.00", "400", "USD", "BUY"],
      ["GEV", "1149.19", "2025-03-01", "600.00", "10", "USD", "BUY"],
      ["GEV", "1149.19", "2025-09-01", "800.00", "5", "USD", "BUY"],
      ["GEV", "1149.19", "2026-04-01", "1100.00", "5", "USD", "SELL"],
      ["FULLY_CLOSED", "100.00", "2025-01-01", "100.00", "50", "USD", "BUY"],
      ["FULLY_CLOSED", "100.00", "2025-06-01", "120.00", "50", "USD", "SELL"],
    ];

    const out = parseHoldingsSnapshots(
      TX_LOG_HEADERS,
      rows,
      HOLDINGS_MAPPING_TX_LOG,
      DEFAULT_PARSE_OPTIONS,
    );

    expect(out).toHaveLength(1);
    expect(out[0].positions).toHaveLength(3); // FULLY_CLOSED dropped

    const aapl = out[0].positions.find((p) => p.symbol === "AAPL")!;
    expect(parseFloat(aapl.quantity)).toBe(85); // 95 - 10
    // Weighted avg over BUYs: (50×210 + 25×240 + 20×275) / 95
    //                       = (10500 + 6000 + 5500) / 95 = 22000 / 95 ≈ 231.58
    expect(parseFloat(aapl.avgCost!)).toBeCloseTo(231.5789, 3);

    const qbts = out[0].positions.find((p) => p.symbol === "QBTS")!;
    expect(parseFloat(qbts.quantity)).toBe(1200);

    const gev = out[0].positions.find((p) => p.symbol === "GEV")!;
    expect(parseFloat(gev.quantity)).toBe(10); // 15 - 5
  });
});

describe("parseHoldingsSnapshots — legacy snapshot mode (no Transaction Type column)", () => {
  it("falls back to legacy 'each row = one position' when no tx type column", () => {
    // No "Transaction Type" header → legacy mode.
    const headers = ["Date", "Symbol", "Quantity", "Avg Cost", "Currency"];
    const mapping: Record<string, string> = {
      [HoldingsFormat.DATE]: "Date",
      [HoldingsFormat.SYMBOL]: "Symbol",
      [HoldingsFormat.QUANTITY]: "Quantity",
      [HoldingsFormat.AVG_COST]: "Avg Cost",
      [HoldingsFormat.CURRENCY]: "Currency",
    };
    const rows = [
      ["2026-05-12", "AAPL", "40", "230.00", "USD"],
      ["2026-05-12", "MSFT", "20", "350.00", "USD"],
    ];

    const out = parseHoldingsSnapshots(headers, rows, mapping, DEFAULT_PARSE_OPTIONS);

    expect(out).toHaveLength(1);
    expect(out[0].date).toBe("2026-05-12");
    expect(out[0].positions).toHaveLength(2);
    expect(out[0].positions.find((p) => p.symbol === "AAPL")?.quantity).toBe("40");
  });
});
