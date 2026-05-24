// Tauri-specific activity commands
import type { CsvImportAnalysis, ParseConfig, ParsedCsvResult } from "@/lib/types";
import { invoke, logger } from "./core";

/**
 * Parse a CSV file with the given configuration.
 * Tauri implementation: reads file as ArrayBuffer and invokes parse_csv command.
 */
export const parseCsv = async (file: File, config: ParseConfig): Promise<ParsedCsvResult> => {
  try {
    const buffer = await file.arrayBuffer();
    const content = Array.from(new Uint8Array(buffer));
    return await invoke<ParsedCsvResult>("parse_csv", { content, config });
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
