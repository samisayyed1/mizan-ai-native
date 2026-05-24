// Tauri-specific activity commands
import type { CsvImportAnalysis, ParseConfig, ParsedCsvResult } from "@/lib/types";
import { invoke, logger } from "./core";

/**
 * Parse a CSV file with the given configuration.
 *
 * Implementation note: the previous version read the file as ArrayBuffer
 * and did `Array.from(new Uint8Array(buffer))` before invoking the
 * `parse_csv` Tauri command. Tauri's IPC bridge JSON-encodes args, so a
 * 5 MB CSV crossed the bridge as ~20 MB of `"[103,105,...]"` text and
 * stalled the browser main thread for tens of seconds — the visible
 * "loading forever" bug. We now read the file as a UTF-8 string
 * (browser-native, no IPC) and send the string to the new
 * `parse_csv_text` command. JSON-encoding a string is ~1× its byte
 * length, not ~4×, and string deserialization on the Rust side is
 * an O(n) memcpy.
 *
 * Non-UTF-8 broker exports are rare enough that the trade-off is worth
 * it; if we ever hit one we surface a clean error instead of hanging.
 */
export const parseCsv = async (file: File, config: ParseConfig): Promise<ParsedCsvResult> => {
  try {
    const text = await file.text();
    return await invoke<ParsedCsvResult>("parse_csv_text", { text, config });
  } catch (err) {
    logger.error("Error parsing CSV file:", err);
    throw err;
  }
};

/**
 * Parse a CSV AND run smart column detection + row filtering +
 * monetary summary in one round-trip. Use this for the import-preview
 * UI so the user sees the headline numbers (total cost basis, rows
 * kept vs dropped, dupes removed) before committing.
 */
export const analyzeCsvImport = async (
  file: File,
  config: ParseConfig,
  sampleSize?: number,
): Promise<CsvImportAnalysis> => {
  try {
    const buffer = await file.arrayBuffer();
    const content = Array.from(new Uint8Array(buffer));
    return await invoke<CsvImportAnalysis>("analyze_csv_import", {
      content,
      config,
      sampleSize,
    });
  } catch (err) {
    logger.error("Error analysing CSV import:", err);
    throw err;
  }
};
