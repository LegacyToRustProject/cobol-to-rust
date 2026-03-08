use anyhow::Result;
use regex::Regex;
use std::path::Path;

use crate::data_division::parse_data_items;
use crate::types::*;

/// Parse a COBOL source file into a structured CobolProgram.
pub fn parse_cobol_source(source: &str) -> Result<CobolProgram> {
    let lines: Vec<&str> = source.lines().collect();

    // Strip sequence numbers (columns 1-6) and indicator (column 7) for fixed format
    let processed: Vec<String> = lines.iter().map(|line| strip_cobol_line(line)).collect();
    let processed_refs: Vec<&str> = processed.iter().map(|s| s.as_str()).collect();

    let program_id = extract_program_id(&processed_refs)?;
    let identification = parse_identification_division(&processed_refs);
    let environment = parse_environment_division(&processed_refs);
    let data = parse_data_division(&processed_refs);
    let procedure = parse_procedure_division(&processed_refs);

    Ok(CobolProgram {
        program_id: program_id.clone(),
        identification,
        environment,
        data,
        procedure,
    })
}

/// Strip COBOL fixed-format columns: columns 1-6 (sequence number) and 7 (indicator).
/// Lines starting with * in column 7 are comments.
fn strip_cobol_line(line: &str) -> String {
    if line.len() <= 6 {
        return String::new();
    }

    let indicator = line.chars().nth(6).unwrap_or(' ');
    if indicator == '*' || indicator == '/' {
        return String::new(); // Comment line
    }

    // Return columns 8-72 (or to end of line if shorter)
    let start = 7.min(line.len());
    let end = 72.min(line.len());
    line[start..end].to_string()
}

fn extract_program_id(lines: &[&str]) -> Result<String> {
    let re = Regex::new(r"(?i)PROGRAM-ID\.\s*([\w-]+)").unwrap();
    for line in lines {
        if let Some(cap) = re.captures(line) {
            return Ok(cap[1].to_string());
        }
    }
    anyhow::bail!("PROGRAM-ID not found in COBOL source")
}

fn parse_identification_division(lines: &[&str]) -> IdentificationDivision {
    let program_id = extract_program_id(lines).unwrap_or_else(|_| "UNKNOWN".to_string());
    let author_re = Regex::new(r"(?i)AUTHOR\.\s*(.+)").unwrap();
    let date_re = Regex::new(r"(?i)DATE-WRITTEN\.\s*(.+)").unwrap();

    let mut author = None;
    let mut date_written = None;

    for line in lines {
        if let Some(cap) = author_re.captures(line) {
            author = Some(cap[1].trim().to_string());
        }
        if let Some(cap) = date_re.captures(line) {
            date_written = Some(cap[1].trim().to_string());
        }
    }

    IdentificationDivision {
        program_id,
        author,
        date_written,
    }
}

fn find_division_range(lines: &[&str], division_name: &str) -> Option<(usize, usize)> {
    let div_re = Regex::new(&format!(r"(?i){}\s+DIVISION", division_name)).unwrap();
    let any_div_re =
        Regex::new(r"(?i)(IDENTIFICATION|ENVIRONMENT|DATA|PROCEDURE)\s+DIVISION").unwrap();

    let mut start = None;

    for (i, line) in lines.iter().enumerate() {
        if div_re.is_match(line) {
            start = Some(i);
        } else if start.is_some() && any_div_re.is_match(line) {
            return Some((start.unwrap(), i));
        }
    }

    start.map(|s| (s, lines.len()))
}

fn parse_environment_division(lines: &[&str]) -> Option<EnvironmentDivision> {
    let (start, end) = find_division_range(lines, "ENVIRONMENT")?;
    let section = &lines[start..end];

    let select_re = Regex::new(r#"(?i)SELECT\s+([\w-]+)\s+ASSIGN\s+TO\s+"?([^"\s.]+)"?"#).unwrap();
    let org_re = Regex::new(r"(?i)ORGANIZATION\s+IS\s+(\w+)").unwrap();

    let mut file_controls = Vec::new();
    let full_text = section.join(" ");

    for cap in select_re.captures_iter(&full_text) {
        let name = cap[1].to_string();
        let assign_to = cap[2].to_string();
        let organization = if let Some(org_cap) = org_re.captures(&full_text) {
            match org_cap[1].to_uppercase().as_str() {
                "INDEXED" => FileOrganization::Indexed,
                "RELATIVE" => FileOrganization::Relative,
                _ => FileOrganization::Sequential,
            }
        } else {
            FileOrganization::Sequential
        };

        file_controls.push(FileControl {
            name,
            assign_to,
            organization,
        });
    }

    Some(EnvironmentDivision { file_controls })
}

fn parse_data_division(lines: &[&str]) -> Option<DataDivision> {
    let (start, end) = find_division_range(lines, "DATA")?;
    let section = &lines[start..end];

    let ws_re = Regex::new(r"(?i)WORKING-STORAGE\s+SECTION").unwrap();
    let mut ws_start = None;

    for (i, line) in section.iter().enumerate() {
        if ws_re.is_match(line) {
            ws_start = Some(i + 1);
        }
    }

    let working_storage = if let Some(ws) = ws_start {
        let ws_lines: Vec<&str> = section[ws..].to_vec();
        parse_data_items(&ws_lines).unwrap_or_default()
    } else {
        Vec::new()
    };

    Some(DataDivision {
        file_section: Vec::new(),
        working_storage,
    })
}

fn parse_procedure_division(lines: &[&str]) -> Option<ProcedureDivision> {
    let (start, end) = find_division_range(lines, "PROCEDURE")?;
    let section = &lines[start + 1..end];

    let mut paragraphs = Vec::new();
    let mut current_name = String::from("MAIN");
    let mut current_statements: Vec<Statement> = Vec::new();

    let para_re = Regex::new(r"^([A-Z][\w-]*)\.\s*$").unwrap();

    for line in section {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if this is a paragraph header
        if let Some(cap) = para_re.captures(trimmed) {
            if !current_statements.is_empty() || current_name != "MAIN" {
                paragraphs.push(Paragraph {
                    name: current_name.clone(),
                    statements: std::mem::take(&mut current_statements),
                });
            }
            current_name = cap[1].to_string();
            continue;
        }

        if let Some(stmt) = parse_statement(trimmed) {
            current_statements.push(stmt);
        }
    }

    // Push final paragraph
    if !current_statements.is_empty() {
        paragraphs.push(Paragraph {
            name: current_name,
            statements: current_statements,
        });
    }

    Some(ProcedureDivision {
        sections: Vec::new(),
        paragraphs,
    })
}

fn parse_statement(line: &str) -> Option<Statement> {
    let upper = line.to_uppercase();
    let trimmed = upper.trim().trim_end_matches('.');

    if trimmed.starts_with("DISPLAY") {
        Some(parse_display_statement(line))
    } else if trimmed.starts_with("MOVE") {
        Some(parse_move_statement(line))
    } else if trimmed.starts_with("COMPUTE") {
        Some(parse_compute_statement(line))
    } else if trimmed.starts_with("ADD") {
        Some(parse_add_statement(line))
    } else if trimmed.starts_with("SUBTRACT") {
        Some(parse_subtract_statement(line))
    } else if trimmed.starts_with("MULTIPLY") {
        Some(parse_multiply_statement(line))
    } else if trimmed.starts_with("PERFORM") {
        Some(parse_perform_statement(line))
    } else if trimmed.starts_with("EXEC SQL") {
        // Single-line EXEC SQL ... END-EXEC
        let sql_start = line.to_uppercase().find("EXEC SQL").unwrap() + 8;
        let sql_end = line.to_uppercase().find("END-EXEC").unwrap_or(line.len());
        let sql_body = line[sql_start..sql_end].trim().trim_end_matches('.').trim();
        Some(Statement::ExecSql(crate::exec_sql::parse_exec_sql(
            sql_body,
        )))
    } else if trimmed == "STOP RUN" {
        Some(Statement::StopRun)
    } else if trimmed == "GOBACK" {
        Some(Statement::GoBack)
    } else if trimmed.is_empty() {
        None
    } else {
        Some(Statement::Unknown(line.trim().to_string()))
    }
}

fn parse_display_statement(line: &str) -> Statement {
    let content = line
        .trim()
        .strip_prefix("DISPLAY")
        .or_else(|| line.trim().strip_prefix("display"))
        .unwrap_or("")
        .trim()
        .trim_end_matches('.');
    let mut items = Vec::new();

    // Simple parser: split on spaces, handle quoted strings
    let mut chars = content.chars().peekable();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '"';

    while let Some(&ch) = chars.peek() {
        chars.next();
        if in_quote {
            if ch == quote_char {
                items.push(DisplayItem::Literal(current.clone()));
                current.clear();
                in_quote = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            quote_char = ch;
        } else if ch == ' ' {
            if !current.is_empty() {
                items.push(DisplayItem::Variable(current.clone()));
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        items.push(DisplayItem::Variable(current));
    }

    Statement::Display(items)
}

fn parse_move_statement(line: &str) -> Statement {
    let re = Regex::new(r"(?i)MOVE\s+(.+?)\s+TO\s+(.+?)\.?\s*$").unwrap();
    if let Some(cap) = re.captures(line.trim()) {
        let from = cap[1].trim().trim_matches('"').to_string();
        let to: Vec<String> = cap[2]
            .split_whitespace()
            .map(|s| s.trim_end_matches('.').to_string())
            .collect();
        Statement::Move(MoveStatement { from, to })
    } else {
        Statement::Unknown(line.trim().to_string())
    }
}

fn parse_compute_statement(line: &str) -> Statement {
    let re = Regex::new(r"(?i)COMPUTE\s+([\w-]+)\s*=\s*(.+?)\.?\s*$").unwrap();
    if let Some(cap) = re.captures(line.trim()) {
        Statement::Compute(ComputeStatement {
            target: cap[1].to_string(),
            expression: cap[2].trim().to_string(),
        })
    } else {
        Statement::Unknown(line.trim().to_string())
    }
}

fn parse_add_statement(line: &str) -> Statement {
    let re = Regex::new(r"(?i)ADD\s+([\w-]+)\s+TO\s+([\w-]+)").unwrap();
    if let Some(cap) = re.captures(line.trim()) {
        Statement::Add(ArithmeticStatement {
            operand: cap[1].to_string(),
            to: cap[2].trim_end_matches('.').to_string(),
            giving: None,
        })
    } else {
        Statement::Unknown(line.trim().to_string())
    }
}

fn parse_subtract_statement(line: &str) -> Statement {
    let re = Regex::new(r"(?i)SUBTRACT\s+([\w-]+)\s+FROM\s+([\w-]+)").unwrap();
    if let Some(cap) = re.captures(line.trim()) {
        Statement::Subtract(ArithmeticStatement {
            operand: cap[1].to_string(),
            to: cap[2].trim_end_matches('.').to_string(),
            giving: None,
        })
    } else {
        Statement::Unknown(line.trim().to_string())
    }
}

fn parse_multiply_statement(line: &str) -> Statement {
    let re = Regex::new(r"(?i)MULTIPLY\s+([\w-]+)\s+BY\s+([\w-]+)").unwrap();
    if let Some(cap) = re.captures(line.trim()) {
        Statement::Multiply(ArithmeticStatement {
            operand: cap[1].to_string(),
            to: cap[2].trim_end_matches('.').to_string(),
            giving: None,
        })
    } else {
        Statement::Unknown(line.trim().to_string())
    }
}

fn parse_perform_statement(line: &str) -> Statement {
    let varying_re = Regex::new(
        r"(?i)PERFORM\s+([\w-]+)(?:\s+THRU\s+([\w-]+))?\s+VARYING\s+([\w-]+)\s+FROM\s+(\w+)\s+BY\s+(\w+)\s+UNTIL\s+(.+?)\.?\s*$"
    ).unwrap();

    let simple_re = Regex::new(r"(?i)PERFORM\s+([\w-]+)(?:\s+THRU\s+([\w-]+))?\s*\.?\s*$").unwrap();
    let times_re = Regex::new(r"(?i)PERFORM\s+([\w-]+)\s+(\w+)\s+TIMES").unwrap();

    let trimmed = line.trim();

    if let Some(cap) = varying_re.captures(trimmed) {
        Statement::Perform(PerformStatement {
            target: PerformTarget::Paragraph(cap[1].to_string()),
            varying: Some(VaryingClause {
                variable: cap[3].to_string(),
                from: cap[4].to_string(),
                by: cap[5].to_string(),
            }),
            until: Some(cap[6].trim_end_matches('.').to_string()),
            times: None,
            thru: cap.get(2).map(|m| m.as_str().to_string()),
        })
    } else if let Some(cap) = times_re.captures(trimmed) {
        Statement::Perform(PerformStatement {
            target: PerformTarget::Paragraph(cap[1].to_string()),
            varying: None,
            until: None,
            times: Some(cap[2].to_string()),
            thru: None,
        })
    } else if let Some(cap) = simple_re.captures(trimmed) {
        Statement::Perform(PerformStatement {
            target: PerformTarget::Paragraph(cap[1].to_string()),
            varying: None,
            until: None,
            times: None,
            thru: cap.get(2).map(|m| m.as_str().to_string()),
        })
    } else {
        Statement::Unknown(trimmed.to_string())
    }
}

/// Analyze a COBOL source file and produce a summary.
pub fn analyze_file(path: &Path, source: &str) -> Result<ProgramSummary> {
    let program = parse_cobol_source(source)?;

    let mut divisions = vec!["IDENTIFICATION".to_string()];
    if program.environment.is_some() {
        divisions.push("ENVIRONMENT".to_string());
    }
    if program.data.is_some() {
        divisions.push("DATA".to_string());
    }
    if program.procedure.is_some() {
        divisions.push("PROCEDURE".to_string());
    }

    let data_items = program
        .data
        .as_ref()
        .map(|d| count_data_items(&d.working_storage))
        .unwrap_or(0);

    let paragraphs = program
        .procedure
        .as_ref()
        .map(|p| p.paragraphs.len())
        .unwrap_or(0);

    let file_io = program
        .environment
        .as_ref()
        .is_some_and(|e| !e.file_controls.is_empty());

    Ok(ProgramSummary {
        file_path: path.to_string_lossy().to_string(),
        program_id: program.program_id,
        divisions,
        data_items,
        paragraphs,
        file_io,
        line_count: source.lines().count(),
    })
}

fn count_data_items(items: &[DataItem]) -> usize {
    items
        .iter()
        .map(|item| 1 + count_data_items(&item.children))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_WORLD: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO-WORLD.
       PROCEDURE DIVISION.
           DISPLAY "HELLO WORLD".
           STOP RUN.
"#;

    #[test]
    fn test_parse_hello_world() {
        let program = parse_cobol_source(HELLO_WORLD).unwrap();
        assert_eq!(program.program_id, "HELLO-WORLD");
        let proc = program.procedure.unwrap();
        assert!(!proc.paragraphs.is_empty());
        let stmts = &proc.paragraphs[0].statements;
        assert!(stmts.len() >= 2);
        assert!(matches!(&stmts[0], Statement::Display(_)));
        assert!(matches!(&stmts[1], Statement::StopRun));
    }

    #[test]
    fn test_parse_display_literal() {
        let stmt = parse_display_statement(r#"DISPLAY "HELLO WORLD"."#);
        if let Statement::Display(items) = stmt {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], DisplayItem::Literal(s) if s == "HELLO WORLD"));
        } else {
            panic!("Expected Display statement");
        }
    }

    #[test]
    fn test_strip_cobol_line() {
        let line = "000100 IDENTIFICATION DIVISION.                                         ";
        let stripped = strip_cobol_line(line);
        assert!(stripped.contains("IDENTIFICATION DIVISION"));
    }

    #[test]
    fn test_strip_comment_line() {
        let line = "000100*THIS IS A COMMENT                                                ";
        let stripped = strip_cobol_line(line);
        assert!(stripped.is_empty());
    }

    #[test]
    fn test_analyze_file() {
        let summary = analyze_file(Path::new("test.cob"), HELLO_WORLD).unwrap();
        assert_eq!(summary.program_id, "HELLO-WORLD");
        assert!(summary.divisions.contains(&"PROCEDURE".to_string()));
        assert!(!summary.file_io);
    }
}
