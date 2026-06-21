//! XLSX → CSV conversion for AI chat attachments.
//!
//! User goal: 'make sure the ai is capable of reading pdf's csv, xlsx'.
//! PDF + CSV land natively (Claude consumes them as `Document` /
//! `Text` parts on the wire); XLSX is binary and can't be sent
//! directly, so the chat dispatcher converts it to CSV in-process
//! before passing it to the model as a regular CSV attachment.
//!
//! Only the first non-empty sheet is converted by default — broker
//! statements + portfolio exports virtually always put the meaningful
//! data on the first sheet. We surface the sheet name in the wrapper
//! so the model knows which one it's looking at.
//!
//! Pure-Rust via [`calamine`]; no Office or COM dependency.

use base64::Engine;
use calamine::{open_workbook_auto_from_rs, Data, Reader};
use std::io::Cursor;

/// MIME types we recognise as XLSX. Both the modern OpenXML format
/// and the legacy binary `.xls` go through the same `calamine` reader.
pub const XLSX_MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-excel",
    "application/vnd.ms-excel.sheet.macroenabled.12",
];

pub fn is_xlsx_mime(content_type: &str) -> bool {
    XLSX_MIME_TYPES.contains(&content_type)
}

/// Decode + convert a base64-encoded XLSX/XLS payload into a CSV
/// string ready to drop into a chat Text part.
///
/// Returns `(sheet_name, csv_text)`. Caps each sheet at `max_rows`
/// to avoid blowing the model's context window with multi-thousand-
/// row broker statements — anything past the cap is truncated with a
/// trailing note.
pub fn xlsx_attachment_to_csv(
    base64_data: &str,
    max_rows: usize,
) -> Result<(String, String), XlsxError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| XlsxError::Base64(e.to_string()))?;

    let cursor = Cursor::new(bytes);
    let mut workbook =
        open_workbook_auto_from_rs(cursor).map_err(|e| XlsxError::Open(e.to_string()))?;

    // Pick the first sheet that has at least one non-empty cell. Some
    // exports leave 'Sheet1' empty and put the data on 'Activities' or
    // 'Holdings' — defaulting to index 0 would silently return blank.
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let mut chosen: Option<(String, calamine::Range<Data>)> = None;
    for name in &sheet_names {
        match workbook.worksheet_range(name) {
            Ok(range) if !range.is_empty() => {
                chosen = Some((name.clone(), range));
                break;
            }
            _ => continue,
        }
    }
    let (sheet_name, range) = chosen.ok_or(XlsxError::NoSheets)?;

    let mut csv = String::new();
    let mut row_count = 0usize;
    let mut truncated = false;
    for row in range.rows() {
        if row_count >= max_rows {
            truncated = true;
            break;
        }
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                csv.push(',');
            }
            let s = cell_to_string(cell);
            // Quote any cell that contains the CSV reserved set so a
            // downstream csv::Reader can parse it without ambiguity.
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                csv.push('"');
                csv.push_str(&s.replace('"', "\"\""));
                csv.push('"');
            } else {
                csv.push_str(&s);
            }
        }
        csv.push('\n');
        row_count += 1;
    }

    if truncated {
        csv.push_str(&format!(
            "[truncated at {} rows — re-upload as CSV if you need the full file]\n",
            max_rows
        ));
    }

    Ok((sheet_name, csv))
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            // Whole numbers render without a trailing .0 so a date
            // serial or share count looks clean to the model.
            if f.fract() == 0.0 && f.is_finite() {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(dt) => format!("{}", dt.as_f64()),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR:{:?}", e),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum XlsxError {
    #[error("XLSX base64 decode failed: {0}")]
    Base64(String),
    #[error("XLSX open failed: {0}")]
    Open(String),
    #[error("XLSX workbook has no non-empty sheets")]
    NoSheets,
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    // Minimal XLSX bytes constructed via rust_xlsxwriter — kept in tests
    // only since rust_xlsxwriter would otherwise bloat the runtime
    // dependency tree. Inline base64 of a 2-column / 3-row sheet
    // ('Activities' tab) so the tests stay hermetic.
    fn fixture_xlsx() -> String {
        let mut wb = rust_xlsxwriter::Workbook::new();
        let sheet = wb.add_worksheet().set_name("Activities").unwrap();
        sheet.write_string(0, 0, "Symbol").unwrap();
        sheet.write_string(0, 1, "Quantity").unwrap();
        sheet.write_string(1, 0, "AAPL").unwrap();
        sheet.write_number(1, 1, 100.0).unwrap();
        sheet.write_string(2, 0, "NVDA").unwrap();
        sheet.write_number(2, 1, 25.0).unwrap();
        let bytes = wb.save_to_buffer().unwrap();
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    }

    #[test]
    fn converts_a_simple_workbook_to_csv() {
        let (sheet_name, csv) = xlsx_attachment_to_csv(&fixture_xlsx(), 1000).unwrap();
        assert_eq!(sheet_name, "Activities");
        assert!(csv.contains("Symbol,Quantity"));
        assert!(csv.contains("AAPL,100"));
        assert!(csv.contains("NVDA,25"));
    }

    #[test]
    fn truncates_at_max_rows() {
        let (_, csv) = xlsx_attachment_to_csv(&fixture_xlsx(), 2).unwrap();
        // 2 rows kept (header + first row), trailing truncate marker.
        assert!(csv.contains("Symbol,Quantity"));
        assert!(csv.contains("AAPL,100"));
        assert!(!csv.contains("NVDA"));
        assert!(csv.contains("[truncated at 2 rows"));
    }

    #[test]
    fn rejects_invalid_base64() {
        let err = xlsx_attachment_to_csv("not-base64!!", 1000).unwrap_err();
        assert!(matches!(err, XlsxError::Base64(_)));
    }

    #[test]
    fn rejects_non_xlsx_bytes() {
        let payload = base64::engine::general_purpose::STANDARD.encode(b"this is not an xlsx file");
        let err = xlsx_attachment_to_csv(&payload, 1000).unwrap_err();
        assert!(matches!(err, XlsxError::Open(_)));
    }

    #[test]
    fn picks_first_non_empty_sheet() {
        let mut wb = rust_xlsxwriter::Workbook::new();
        // Sheet1 left empty.
        wb.add_worksheet().set_name("Sheet1").unwrap();
        let sheet = wb.add_worksheet().set_name("Holdings").unwrap();
        sheet.write_string(0, 0, "Ticker").unwrap();
        sheet.write_string(1, 0, "MSFT").unwrap();
        let payload = base64::engine::general_purpose::STANDARD.encode(&wb.save_to_buffer().unwrap());
        let (sheet_name, csv) = xlsx_attachment_to_csv(&payload, 1000).unwrap();
        assert_eq!(sheet_name, "Holdings");
        assert!(csv.contains("Ticker"));
        assert!(csv.contains("MSFT"));
    }

    #[test]
    fn quotes_cells_with_csv_metachars() {
        let mut wb = rust_xlsxwriter::Workbook::new();
        let sheet = wb.add_worksheet().set_name("Notes").unwrap();
        sheet.write_string(0, 0, "Asset").unwrap();
        sheet.write_string(0, 1, "Note").unwrap();
        sheet
            .write_string(1, 0, "Apple, Inc.")
            .unwrap();
        sheet
            .write_string(1, 1, "split 4:1 - \"big news\"")
            .unwrap();
        let payload = base64::engine::general_purpose::STANDARD.encode(&wb.save_to_buffer().unwrap());
        let (_, csv) = xlsx_attachment_to_csv(&payload, 1000).unwrap();
        // The comma in the company name should be quoted; the embedded
        // " escaped to "".
        assert!(csv.contains("\"Apple, Inc.\""));
        assert!(csv.contains("\"split 4:1 - \"\"big news\"\"\""));
    }

    #[test]
    fn is_xlsx_mime_matches_official_set() {
        assert!(is_xlsx_mime(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        ));
        assert!(is_xlsx_mime("application/vnd.ms-excel"));
        assert!(!is_xlsx_mime("text/csv"));
        assert!(!is_xlsx_mime("application/pdf"));
    }
}
