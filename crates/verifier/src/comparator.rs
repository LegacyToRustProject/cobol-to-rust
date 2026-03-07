use anyhow::Result;

/// Result of comparing COBOL and Rust program outputs.
#[derive(Debug)]
pub struct ComparisonResult {
    pub matches: bool,
    pub expected: String,
    pub actual: String,
    pub diff_lines: Vec<DiffLine>,
}

#[derive(Debug)]
pub struct DiffLine {
    pub line_number: usize,
    pub expected: String,
    pub actual: String,
}

/// Compare expected output (from COBOL) with actual output (from Rust).
/// Normalizes trailing whitespace and line endings.
pub fn compare_outputs(expected: &str, actual: &str) -> ComparisonResult {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();

    let mut diff_lines = Vec::new();
    let max_lines = expected_lines.len().max(actual_lines.len());

    for i in 0..max_lines {
        let exp = expected_lines.get(i).map(|s| s.trim_end()).unwrap_or("");
        let act = actual_lines.get(i).map(|s| s.trim_end()).unwrap_or("");

        if exp != act {
            diff_lines.push(DiffLine {
                line_number: i + 1,
                expected: exp.to_string(),
                actual: act.to_string(),
            });
        }
    }

    ComparisonResult {
        matches: diff_lines.is_empty(),
        expected: expected.to_string(),
        actual: actual.to_string(),
        diff_lines,
    }
}

/// Format a comparison result as a human-readable diff.
pub fn format_diff(result: &ComparisonResult) -> String {
    if result.matches {
        return "Output matches!".to_string();
    }

    let mut output = format!(
        "Output mismatch: {} line(s) differ\n\n",
        result.diff_lines.len()
    );

    for diff in &result.diff_lines {
        output.push_str(&format!("Line {}:\n", diff.line_number));
        output.push_str(&format!("  Expected: {:?}\n", diff.expected));
        output.push_str(&format!("  Actual:   {:?}\n", diff.actual));
    }

    output
}

/// Compare outputs from a file containing the expected output.
pub fn compare_with_file(
    expected_path: &std::path::Path,
    actual: &str,
) -> Result<ComparisonResult> {
    let expected = std::fs::read_to_string(expected_path)?;
    Ok(compare_outputs(&expected, actual))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matching_outputs() {
        let result = compare_outputs("HELLO WORLD\n", "HELLO WORLD\n");
        assert!(result.matches);
        assert!(result.diff_lines.is_empty());
    }

    #[test]
    fn test_mismatching_outputs() {
        let result = compare_outputs("HELLO WORLD\n", "HELLO RUST\n");
        assert!(!result.matches);
        assert_eq!(result.diff_lines.len(), 1);
        assert_eq!(result.diff_lines[0].line_number, 1);
    }

    #[test]
    fn test_trailing_whitespace_ignored() {
        let result = compare_outputs("HELLO   \n", "HELLO\n");
        assert!(result.matches);
    }

    #[test]
    fn test_different_line_counts() {
        let result = compare_outputs("LINE 1\nLINE 2\n", "LINE 1\n");
        assert!(!result.matches);
        assert_eq!(result.diff_lines.len(), 1);
    }

    #[test]
    fn test_format_diff_match() {
        let result = compare_outputs("ok\n", "ok\n");
        let formatted = format_diff(&result);
        assert!(formatted.contains("matches"));
    }

    #[test]
    fn test_format_diff_mismatch() {
        let result = compare_outputs("expected\n", "actual\n");
        let formatted = format_diff(&result);
        assert!(formatted.contains("mismatch"));
    }
}
