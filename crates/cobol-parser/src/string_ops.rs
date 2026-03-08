/// STRING/UNSTRING statement conversion to Rust equivalents.
///
/// COBOL STRING concatenates values with optional delimiters:
/// ```text
/// STRING WS-FIRST DELIMITED BY SPACE
///        WS-LAST  DELIMITED BY SIZE
///        INTO WS-FULL-NAME.
/// ```
/// → Rust: `format!("{} {}", ws_first.trim(), ws_last)`
///
/// COBOL UNSTRING splits a value on delimiters:
/// ```text
/// UNSTRING WS-INPUT DELIMITED BY ","
///          INTO WS-FIELD1, WS-FIELD2, WS-FIELD3.
/// ```
/// → Rust: `let parts: Vec<&str> = ws_input.splitn(3, ',').collect();`
/// Describes one source field in a STRING statement.
#[derive(Debug, Clone, PartialEq)]
pub struct StringSource {
    pub variable: String,
    pub delimiter: StringDelimiter,
}

/// Delimiter type for STRING statement sources.
#[derive(Debug, Clone, PartialEq)]
pub enum StringDelimiter {
    /// `DELIMITED BY SPACE` — trim trailing spaces before concatenating
    Space,
    /// `DELIMITED BY SIZE` — use full field value without trimming
    Size,
    /// `DELIMITED BY literal` — stop at the first occurrence of the literal
    Literal(String),
}

/// Parsed STRING statement.
#[derive(Debug, Clone)]
pub struct StringStatement {
    pub sources: Vec<StringSource>,
    pub into_var: String,
}

impl StringStatement {
    /// Generate the Rust `format!` expression for this STRING statement.
    pub fn to_rust_format(&self) -> String {
        let parts: Vec<String> = self
            .sources
            .iter()
            .map(|src| {
                let field = to_rust_field_name(&src.variable);
                match &src.delimiter {
                    StringDelimiter::Space => "{}".to_string(),
                    StringDelimiter::Size => "{}".to_string(),
                    StringDelimiter::Literal(_) => "{}".to_string(),
                }
                .replace(
                    "{}",
                    &format!(
                        "{{ {} }}",
                        match &src.delimiter {
                            StringDelimiter::Space => format!("{}.trim_end()", field),
                            StringDelimiter::Size => field.clone(),
                            StringDelimiter::Literal(lit) => {
                                format!("{}.split({:?}).next().unwrap_or(\"\")", field, lit)
                            }
                        }
                    ),
                )
            })
            .collect();

        let target = to_rust_field_name(&self.into_var);
        format!(
            "let {} = format!(\"{}\", {});",
            target,
            parts.iter().map(|_| "{}").collect::<Vec<_>>().join(""),
            self.sources
                .iter()
                .map(|src| {
                    let field = to_rust_field_name(&src.variable);
                    match &src.delimiter {
                        StringDelimiter::Space => format!("{}.trim_end()", field),
                        StringDelimiter::Size => field.clone(),
                        StringDelimiter::Literal(lit) => {
                            format!("{}.split({:?}).next().unwrap_or(\"\")", field, lit)
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Parsed UNSTRING statement.
#[derive(Debug, Clone)]
pub struct UnstringStatement {
    pub source_var: String,
    pub delimiter: String,
    pub into_vars: Vec<String>,
}

impl UnstringStatement {
    /// Generate the Rust `splitn` expression for this UNSTRING statement.
    pub fn to_rust_splitn(&self) -> String {
        let source = to_rust_field_name(&self.source_var);
        let n = self.into_vars.len();
        let parts_var = format!("{}_parts", source);
        let delimiter = &self.delimiter;

        let assignments: String = self
            .into_vars
            .iter()
            .enumerate()
            .map(|(i, var)| {
                let field = to_rust_field_name(var);
                format!(
                    "let {} = {}.get({}).copied().unwrap_or(\"\").trim().to_string();",
                    field, parts_var, i
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "let {parts_var}: Vec<&str> = {source}.splitn({n}, {delim:?}).collect();\n{assignments}",
            parts_var = parts_var,
            source = source,
            n = n,
            delim = delimiter,
            assignments = assignments
        )
    }
}

/// Parse a COBOL STRING statement text into a `StringStatement`.
///
/// Handles: `STRING src1 DELIMITED BY SPACE src2 DELIMITED BY SIZE INTO target`
pub fn parse_string_statement(line: &str) -> Option<StringStatement> {
    let upper = line.to_uppercase();
    let trimmed = upper.trim().trim_end_matches('.');

    if !trimmed.starts_with("STRING ") {
        return None;
    }

    // Split on INTO to get sources portion and target
    let into_pos = trimmed.find(" INTO ")?;
    let sources_part = &trimmed[7..into_pos]; // skip "STRING "
    let _into_var = trimmed[into_pos + 6..].trim().to_string();
    // Convert target back to lowercase for original case
    let into_var_orig = {
        let pos = line.to_uppercase().find(" INTO ").unwrap() + 6;
        line[pos..].trim().trim_end_matches('.').trim().to_string()
    };

    // Parse sources: VAR DELIMITED BY {SPACE|SIZE|literal}
    let mut sources = Vec::new();
    // Split on DELIMITED to pair up variable + delimiter
    let tokens: Vec<&str> = sources_part.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let var_token = tokens[i];
        i += 1;

        // Look for DELIMITED BY
        if i + 2 < tokens.len()
            && tokens[i].eq_ignore_ascii_case("DELIMITED")
            && tokens[i + 1].eq_ignore_ascii_case("BY")
        {
            i += 2;
            let delim_token = tokens[i];
            i += 1;
            let delimiter = if delim_token.eq_ignore_ascii_case("SPACE") {
                StringDelimiter::Space
            } else if delim_token.eq_ignore_ascii_case("SIZE") {
                StringDelimiter::Size
            } else {
                StringDelimiter::Literal(delim_token.trim_matches('"').to_string())
            };
            sources.push(StringSource {
                variable: var_token.to_string(),
                delimiter,
            });
        } else {
            // No delimiter — treat as SIZE
            sources.push(StringSource {
                variable: var_token.to_string(),
                delimiter: StringDelimiter::Size,
            });
        }
    }

    Some(StringStatement {
        sources,
        into_var: into_var_orig,
    })
}

/// Parse a COBOL UNSTRING statement text into an `UnstringStatement`.
///
/// Handles: `UNSTRING src DELIMITED BY "," INTO var1, var2, var3`
pub fn parse_unstring_statement(line: &str) -> Option<UnstringStatement> {
    let upper = line.to_uppercase();
    let trimmed = upper.trim().trim_end_matches('.');

    if !trimmed.starts_with("UNSTRING ") {
        return None;
    }

    // Split on DELIMITED BY and INTO
    let del_pos = trimmed.find(" DELIMITED BY ")?;
    let into_pos = trimmed.find(" INTO ")?;

    let _source_var = trimmed[9..del_pos].trim().to_string(); // skip "UNSTRING "
    let delimiter_raw = trimmed[del_pos + 14..into_pos].trim().to_string();
    let delimiter = delimiter_raw.trim_matches('"').to_string();
    let into_part = &trimmed[into_pos + 6..];

    // Original-case into_vars
    let orig_into_pos = {
        let p = line.to_uppercase().find(" INTO ").unwrap();
        p + 6
    };
    let into_vars_orig: Vec<String> = line[orig_into_pos..]
        .trim()
        .trim_end_matches('.')
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Original-case source_var
    let orig_del_pos = line.to_uppercase().find(" DELIMITED BY ").unwrap();
    let source_var_orig = line[9..orig_del_pos].trim().to_string();

    let _ = into_part; // suppress unused warning

    Some(UnstringStatement {
        source_var: source_var_orig,
        delimiter,
        into_vars: into_vars_orig,
    })
}

/// Convert a COBOL hyphenated name to Rust snake_case.
fn to_rust_field_name(name: &str) -> String {
    name.replace('-', "_").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_stmt_space_delimiter() {
        let stmt = parse_string_statement(
            "STRING WS-FIRST DELIMITED BY SPACE WS-LAST DELIMITED BY SIZE INTO WS-FULL.",
        )
        .unwrap();
        assert_eq!(stmt.sources.len(), 2);
        assert_eq!(stmt.sources[0].variable, "WS-FIRST");
        assert_eq!(stmt.sources[0].delimiter, StringDelimiter::Space);
        assert_eq!(stmt.sources[1].variable, "WS-LAST");
        assert_eq!(stmt.sources[1].delimiter, StringDelimiter::Size);
        assert_eq!(stmt.into_var, "WS-FULL");
    }

    #[test]
    fn test_string_stmt_rust_format_contains_trim() {
        let stmt = parse_string_statement(
            "STRING WS-FIRST DELIMITED BY SPACE WS-LAST DELIMITED BY SIZE INTO WS-FULL.",
        )
        .unwrap();
        let code = stmt.to_rust_format();
        // DELIMITED BY SPACE should use trim_end()
        assert!(
            code.contains("trim_end()"),
            "SPACE delimiter should call trim_end()"
        );
        assert!(
            code.contains("ws_full"),
            "target variable should appear in output"
        );
        assert!(code.contains("format!"), "should use format! macro");
    }

    #[test]
    fn test_string_stmt_literal_delimiter() {
        let stmt = parse_string_statement(
            "STRING WS-CODE DELIMITED BY \",\" WS-DESC DELIMITED BY SIZE INTO WS-RESULT.",
        )
        .unwrap();
        assert_eq!(
            stmt.sources[0].delimiter,
            StringDelimiter::Literal(",".to_string())
        );
    }

    #[test]
    fn test_unstring_stmt_parsed() {
        let stmt = parse_unstring_statement(
            "UNSTRING WS-INPUT DELIMITED BY \",\" INTO WS-F1, WS-F2, WS-F3.",
        )
        .unwrap();
        assert_eq!(stmt.source_var, "WS-INPUT");
        assert_eq!(stmt.delimiter, ",");
        assert_eq!(stmt.into_vars, vec!["WS-F1", "WS-F2", "WS-F3"]);
    }

    #[test]
    fn test_unstring_rust_splitn_code() {
        let stmt = UnstringStatement {
            source_var: "WS-INPUT".to_string(),
            delimiter: ",".to_string(),
            into_vars: vec!["WS-F1".to_string(), "WS-F2".to_string()],
        };
        let code = stmt.to_rust_splitn();
        assert!(
            code.contains("splitn(2"),
            "should use splitn with count = number of output vars"
        );
        assert!(code.contains("ws_input"), "source variable in snake_case");
        assert!(code.contains("ws_f1"), "first output var in snake_case");
        assert!(code.contains("ws_f2"), "second output var in snake_case");
    }

    #[test]
    fn test_unstring_rust_splitn_correct_count() {
        let stmt = UnstringStatement {
            source_var: "WS-LINE".to_string(),
            delimiter: "|".to_string(),
            into_vars: vec![
                "WS-COL1".to_string(),
                "WS-COL2".to_string(),
                "WS-COL3".to_string(),
            ],
        };
        let code = stmt.to_rust_splitn();
        assert!(
            code.contains("splitn(3"),
            "should splitn with 3 for 3 output vars"
        );
        assert!(code.contains("ws_col3"), "third column should appear");
    }
}
