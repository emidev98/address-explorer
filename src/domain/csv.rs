//! Shared RFC 4180-style CSV tokenizer used by the exchange ledger importer
//! and the catalog importer. It is intentionally independent from HTTP,
//! SQLite, and Dioxus so it can be unit-tested on its own.
use std::collections::BTreeMap;
/// Parsed CSV document: the original headers and one map per data row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvDocument {
    pub headers: Vec<String>,
    pub rows: Vec<BTreeMap<String, String>>,
}
/// Parse RFC 4180-style CSV text with quoted fields, escaped quotes, CRLF,
/// and newlines inside quoted fields. Blank lines are skipped and a missing
/// trailing cell is read as an empty string.
pub fn parse_csv_text(text: &str) -> Result<CsvDocument, String> {
    let text = text.trim_start_matches('\u{feff}').trim_end();
    if text.is_empty() {
        return Err("The selected file is empty.".to_string());
    }
    let lines = split_csv_lines(text);
    let Some(header_line) = lines.first() else {
        return Err("The selected file has no header row.".to_string());
    };
    let headers = parse_csv_line(header_line);
    if headers.iter().all(|header| header.trim().is_empty()) {
        return Err("The selected file has no header row.".to_string());
    }
    let mut rows = Vec::new();
    for line in lines.iter().skip(1) {
        let values = parse_csv_line(line);
        if values.iter().all(|value| value.is_empty()) {
            continue;
        }
        let mut row = BTreeMap::new();
        for (index, header) in headers.iter().enumerate() {
            row.insert(
                header.clone(),
                values.get(index).cloned().unwrap_or_default(),
            );
        }
        rows.push(row);
    }
    Ok(CsvDocument { headers, rows })
}
fn split_csv_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                current.push('"');
                chars.next();
            }
            '"' => {
                current.push(character);
                in_quotes = !in_quotes;
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            '\n' if !in_quotes => lines.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_tokenizer_handles_quotes_crlf_and_embedded_newlines() {
        let text = "A,B\r\n\"x,1\",\"hello\r\nworld\"\r\n\"a\"\"b\",plain\r\n";
        let document = parse_csv_text(text).unwrap();
        assert_eq!(document.headers, ["A", "B"]);
        assert_eq!(document.rows.len(), 2);
        assert_eq!(document.rows[0]["A"], "x,1");
        assert_eq!(document.rows[0]["B"], "hello\r\nworld");
        assert_eq!(document.rows[1]["A"], "a\"b");
    }
    #[test]
    fn a_bom_and_blank_lines_are_ignored() {
        let document = parse_csv_text("\u{feff}name,category\r\n\r\nLedger,\r\n").unwrap();
        assert_eq!(document.headers, ["name", "category"]);
        assert_eq!(document.rows.len(), 1);
        assert_eq!(document.rows[0]["name"], "Ledger");
        assert_eq!(document.rows[0]["category"], "");
    }
    #[test]
    fn empty_files_and_missing_headers_are_rejected() {
        assert!(parse_csv_text("   ").is_err());
        assert!(parse_csv_text("\n\n").is_err());
    }
}
