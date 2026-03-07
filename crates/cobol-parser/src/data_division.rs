use anyhow::{Context, Result};
use regex::Regex;

use crate::types::{DataItem, PicClause, PicType};

/// Parse a PIC clause string into a structured PicClause.
///
/// Examples:
/// - `9(7)V99` → 7 integer digits + 2 decimal digits, numeric
/// - `X(30)` → 30 chars, alphanumeric
/// - `S9(5)V99` → signed, 5 integer + 2 decimal
/// - `9` → 1 digit numeric
pub fn parse_pic_clause(raw: &str) -> Result<PicClause> {
    let raw_trimmed = raw.trim().trim_end_matches('.');
    let upper = raw_trimmed.to_uppercase();

    let signed = upper.starts_with('S');
    let pic_str = if signed { &upper[1..] } else { &upper };

    // Determine type
    let pic_type = if pic_str.contains('X') || pic_str.contains('A') {
        if pic_str.contains('X') {
            PicType::Alphanumeric
        } else {
            PicType::Alphabetic
        }
    } else {
        PicType::Numeric
    };

    match pic_type {
        PicType::Alphanumeric | PicType::Alphabetic => {
            let size = count_pic_chars(pic_str);
            Ok(PicClause {
                raw: raw_trimmed.to_string(),
                pic_type,
                integer_digits: 0,
                decimal_digits: 0,
                signed: false,
                total_size: size,
            })
        }
        PicType::Numeric => {
            let (integer_digits, decimal_digits) = parse_numeric_pic(pic_str)?;
            let total_size = integer_digits + decimal_digits;
            Ok(PicClause {
                raw: raw_trimmed.to_string(),
                pic_type,
                integer_digits,
                decimal_digits,
                signed,
                total_size,
            })
        }
    }
}

/// Count repeated PIC characters, expanding `X(n)` notation.
fn count_pic_chars(s: &str) -> u32 {
    let re = Regex::new(r"([XA9])(?:\((\d+)\))?").unwrap();
    let mut total = 0u32;
    for cap in re.captures_iter(s) {
        if let Some(count) = cap.get(2) {
            total += count.as_str().parse::<u32>().unwrap_or(1);
        } else {
            total += 1;
        }
    }
    total
}

/// Parse numeric PIC like `9(7)V99`, `9(5)`, `99V9(3)`.
fn parse_numeric_pic(s: &str) -> Result<(u32, u32)> {
    let parts: Vec<&str> = s.split('V').collect();

    let integer_digits = count_pic_chars(parts[0]);
    let decimal_digits = if parts.len() > 1 {
        count_pic_chars(parts[1])
    } else {
        0
    };

    Ok((integer_digits, decimal_digits))
}

/// Parse DATA DIVISION lines into a list of DataItems.
pub fn parse_data_items(lines: &[&str]) -> Result<Vec<DataItem>> {
    let mut items: Vec<DataItem> = Vec::new();
    let item_re =
        Regex::new(r"^\s*(\d{2})\s+([\w-]+)(?:\s+PIC(?:TURE)?\s+IS\s+)?(?:\s+PIC(?:TURE)?\s+)?([\w()VS.]*)?(?:\s+VALUE\s+(?:IS\s+)?(.+?))?(?:\s+REDEFINES\s+([\w-]+))?\s*\.\s*$")
            .context("Failed to compile data item regex")?;

    // Simpler regex for more robust parsing
    let simple_re = Regex::new(
        r"(?i)^\s*(\d{2})\s+([\w-]+)(?:\s+(?:PIC(?:TURE)?(?:\s+IS)?)\s+([\w()VS.]+))?(?:\s+REDEFINES\s+([\w-]+))?(?:\s+VALUE\s+(?:IS\s+)?(.+?))?\s*\.\s*$"
    ).context("Failed to compile simple regex")?;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            continue;
        }

        if let Some(cap) = simple_re.captures(trimmed) {
            let level: u8 = cap[1].parse().unwrap_or(0);
            let name = cap[2].to_string();
            let picture = cap.get(3).and_then(|m| {
                let pic_str = m.as_str();
                if pic_str.is_empty() {
                    None
                } else {
                    parse_pic_clause(pic_str).ok()
                }
            });
            let redefines = cap.get(4).map(|m| m.as_str().to_string());
            let value = cap.get(5).map(|m| {
                m.as_str()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim()
                    .to_string()
            });

            items.push(DataItem {
                level,
                name,
                picture,
                value,
                redefines,
                children: Vec::new(),
            });
        }
    }

    // Build hierarchy based on level numbers
    let _ = item_re; // suppress unused warning
    build_hierarchy(items)
}

/// Build a hierarchy of DataItems based on COBOL level numbers.
/// Level 01 is top-level, 05/10/15/etc are children.
fn build_hierarchy(flat_items: Vec<DataItem>) -> Result<Vec<DataItem>> {
    if flat_items.is_empty() {
        return Ok(Vec::new());
    }

    let mut result: Vec<DataItem> = Vec::new();
    let mut stack: Vec<DataItem> = Vec::new();

    for item in flat_items {
        // Pop items from stack that are at same or deeper level
        while let Some(top) = stack.last() {
            if top.level >= item.level {
                let popped = stack.pop().unwrap();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(popped);
                } else {
                    result.push(popped);
                }
            } else {
                break;
            }
        }
        stack.push(item);
    }

    // Flush remaining stack
    while let Some(popped) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(popped);
        } else {
            result.push(popped);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pic_numeric_integer() {
        let pic = parse_pic_clause("9(7)").unwrap();
        assert_eq!(pic.integer_digits, 7);
        assert_eq!(pic.decimal_digits, 0);
        assert!(!pic.signed);
        assert!(matches!(pic.pic_type, PicType::Numeric));
    }

    #[test]
    fn test_parse_pic_numeric_with_decimal() {
        let pic = parse_pic_clause("9(7)V99").unwrap();
        assert_eq!(pic.integer_digits, 7);
        assert_eq!(pic.decimal_digits, 2);
        assert!(!pic.signed);
    }

    #[test]
    fn test_parse_pic_signed() {
        let pic = parse_pic_clause("S9(5)V99").unwrap();
        assert_eq!(pic.integer_digits, 5);
        assert_eq!(pic.decimal_digits, 2);
        assert!(pic.signed);
    }

    #[test]
    fn test_parse_pic_alphanumeric() {
        let pic = parse_pic_clause("X(30)").unwrap();
        assert!(matches!(pic.pic_type, PicType::Alphanumeric));
        assert_eq!(pic.total_size, 30);
    }

    #[test]
    fn test_parse_pic_single_digit() {
        let pic = parse_pic_clause("9").unwrap();
        assert_eq!(pic.integer_digits, 1);
        assert_eq!(pic.decimal_digits, 0);
        assert_eq!(pic.total_size, 1);
    }

    #[test]
    fn test_parse_pic_repeated_digits() {
        let pic = parse_pic_clause("999V99").unwrap();
        assert_eq!(pic.integer_digits, 3);
        assert_eq!(pic.decimal_digits, 2);
    }
}
