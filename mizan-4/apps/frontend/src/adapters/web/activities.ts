// Web-specific activity commands
import type { CsvImportAnalysis, ParseConfig, ParsedCsvResult } from "@/lib/types";
import { API_PREFIX, logger } from "./core";

async function extractErrorMessage(response: Response): Promise<string | null> {
  const contentType = response.headers.get("content-type") ?? "";

  if (contentType.includes("application/json")) {
    try {
      const payload = (await response.json()) as {
        message?: unknown;
        error?: unknown;
      };
      if (typeof payload.message === "string" && payload.message.trim()) {
        return payload.message.trim();
      }
      if (typeof payload.error === "string" && payload.error.trim()) {
        return payload.error.trim();
      }
    } catch {
      // Fall through to text parsing
    }
  }

  try {
    const text = (await response.text()).trim();
    return text || null;
  } catch {
    return null;
  }
}

/**
 * Parse a CSV file with the given configuration.
 * Web implementation: POSTs multipart form data to /api/v1/activities/import/parse.
 */
export const parseCsv = async (file: File, config: ParseConfig): Promise<ParsedCsvResult> => {
  try {
    const formData = new FormData();
    formData.append("file", file);
    formData.append("config", JSON.stringify(config));

    const response = await fetch(`${API_PREFIX}/activities/import/parse`, {
      method: "POST",
      body: formData,
      credentials: "same-origin",
    });

    if (!response.ok) {
      const details = await extractErrorMessage(response);
      const fallback = `Request failed (${response.status}${response.statusText ? ` ${response.statusText}` : ""})`;
      throw new Error(
        details ? `Failed to parse CSV: ${details}` : `Failed to parse CSV: ${fallback}`,
      );
    }

    const parsed = (await response.json()) as ParsedCsvResult;
    return parsed;
  } catch (err) {
    logger.error("Error parsing CSV file:", err);
    throw err;
  }
};

/**
 * Web fallback for the smart CSV analysis flow. The server-side
 * endpoint that mirrors `analyze_csv_import` hasn't been wired up
 * yet; until it does, web-mode callers fall back to the structural
 * `parseCsv` and get a degraded but functional CsvImportAnalysis.
 *
 * Previously this threw at runtime — that crashed the import wizard
 * the moment a web user dropped a CSV. Now it parses the rows with
 * the structural parser, returns them with empty `detectedMappings`
 * and a clear `unavailable` summary, and the UI shows a polite
 * "smart mapping is desktop-only — drop the file into Mizan Desktop
 * for auto-detect" card alongside the manual column-mapping step.
 */
export const analyzeCsvImport = async (
  file: File,
  config: ParseConfig,
  _sampleSize?: number,
): Promise<CsvImportAnalysis> => {
  const parsed = await parseCsv(file, config);
  // Degraded but well-shaped result so the import wizard renders.
  // The UI checks `field_mappings` being empty to decide whether to
  // show the "Smart analysis is available on Mizan Desktop — map
  // columns by hand and proceed" hint above the manual mapping step.
  return {
    headers: parsed.headers,
    sample_rows: parsed.rows.slice(0, 50),
    field_mappings: {},
    summary: {
      stats: {
        rows_kept: 0,
        rows_skipped_blank: 0,
        rows_skipped_unparseable: 0,
        rows_total: parsed.rows.length,
      } as unknown as CsvImportAnalysis["summary"]["stats"],
      total_buy_cost_basis: 0,
      total_sell_proceeds: 0,
      total_fees: 0,
      unique_symbols: 0,
      symbols_with_net_position: 0,
      buy_count: 0,
      sell_count: 0,
      other_count: 0,
    },
  };
};
