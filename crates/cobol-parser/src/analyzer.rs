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

    let fs_re = Regex::new(r"(?i)FILE\s+SECTION").unwrap();
    let ws_re = Regex::new(r"(?i)WORKING-STORAGE\s+SECTION").unwrap();
    let fd_re = Regex::new(r"(?i)^\s*FD\s+([\w-]+)").unwrap();

    let mut fs_start = None;
    let mut ws_start = None;

    for (i, line) in section.iter().enumerate() {
        if fs_re.is_match(line) {
            fs_start = Some(i + 1);
        }
        if ws_re.is_match(line) {
            ws_start = Some(i + 1);
        }
    }

    // Parse FILE SECTION: collect FD (file description) names + RECORD/BLOCK CONTAINS
    let record_contains_re = Regex::new(r"(?i)RECORD\s+CONTAINS\s+(\d+)\s+CHARACTERS?").unwrap();
    let block_contains_re = Regex::new(r"(?i)BLOCK\s+CONTAINS\s+(\d+)\s+RECORDS?").unwrap();

    let file_section = if let Some(fs) = fs_start {
        let fs_end = ws_start.unwrap_or(section.len());
        let fs_lines = &section[fs..fs_end];
        let mut fds: Vec<FileDescription> = Vec::new();
        let mut i = 0;
        while i < fs_lines.len() {
            let line = fs_lines[i];
            if let Some(cap) = fd_re.captures(line) {
                let fd_name = cap[1].to_string();
                let mut record_len: Option<usize> = None;
                let mut block_contains: Option<usize> = None;
                // Look ahead for RECORD/BLOCK CONTAINS on subsequent lines
                let mut j = i + 1;
                while j < fs_lines.len() && !fd_re.is_match(fs_lines[j]) {
                    // Stop at level-01 data items (they begin the record layout)
                    let trimmed = fs_lines[j].trim();
                    if trimmed.starts_with("01 ") || trimmed.starts_with("01  ") {
                        break;
                    }
                    if let Some(rc) = record_contains_re.captures(fs_lines[j]) {
                        record_len = rc[1].parse().ok();
                    }
                    if let Some(bc) = block_contains_re.captures(fs_lines[j]) {
                        block_contains = bc[1].parse().ok();
                    }
                    j += 1;
                }
                // Also check the FD line itself for inline RECORD CONTAINS
                if record_len.is_none() {
                    if let Some(rc) = record_contains_re.captures(line) {
                        record_len = rc[1].parse().ok();
                    }
                }
                if block_contains.is_none() {
                    if let Some(bc) = block_contains_re.captures(line) {
                        block_contains = bc[1].parse().ok();
                    }
                }
                fds.push(FileDescription {
                    fd_name,
                    record_len,
                    block_contains,
                    record: Vec::new(),
                });
            }
            i += 1;
        }
        fds
    } else {
        Vec::new()
    };

    let working_storage = if let Some(ws) = ws_start {
        let ws_lines: Vec<&str> = section[ws..].to_vec();
        parse_data_items(&ws_lines).unwrap_or_default()
    } else {
        Vec::new()
    };

    Some(DataDivision {
        file_section,
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
    // COMPUTE [target] [ROUNDED] = [expression]
    let re = Regex::new(r"(?i)COMPUTE\s+([\w-]+)(\s+ROUNDED)?\s*=\s*(.+?)\.?\s*$").unwrap();
    if let Some(cap) = re.captures(line.trim()) {
        let rounded = cap.get(2).is_some();
        Statement::Compute(ComputeStatement {
            target: cap[1].to_string(),
            expression: cap[3].trim().to_string(),
            rounded,
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

    let has_file_controls = program
        .environment
        .as_ref()
        .is_some_and(|e| !e.file_controls.is_empty());
    let has_file_section = program
        .data
        .as_ref()
        .is_some_and(|d| !d.file_section.is_empty());
    let file_io = has_file_controls || has_file_section;

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

    // --- file_io detection ---

    const WITH_FILE_CONTROL: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILE-READ.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT INPUT-FILE ASSIGN TO "input.dat".
       DATA DIVISION.
       FILE SECTION.
       FD INPUT-FILE.
       01 INPUT-REC PIC X(80).
       WORKING-STORAGE SECTION.
       01 WS-EOF PIC 9 VALUE 0.
       PROCEDURE DIVISION.
           STOP RUN.
"#;

    const WITH_FILE_SECTION_ONLY: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. BATCHUPD.
       DATA DIVISION.
       FILE SECTION.
       FD  INPUT-FILE.
       01  INPUT-RECORD  PIC X(21).
       FD  OUTPUT-FILE.
       01  OUTPUT-RECORD PIC X(80).
       WORKING-STORAGE SECTION.
       01  WS-EOF PIC 9 VALUE 0.
       PROCEDURE DIVISION.
           STOP RUN.
"#;

    #[test]
    fn test_file_io_detected_via_file_control() {
        let summary = analyze_file(Path::new("test.cob"), WITH_FILE_CONTROL).unwrap();
        assert!(
            summary.file_io,
            "file_io should be true when FILE-CONTROL is present"
        );
    }

    #[test]
    fn test_file_io_detected_via_file_section_only() {
        // Programs like BATCHUPD define FILE SECTION without ENVIRONMENT/FILE-CONTROL.
        // The parser should still detect file I/O from the FD declarations.
        let summary = analyze_file(Path::new("test.cob"), WITH_FILE_SECTION_ONLY).unwrap();
        assert!(
            summary.file_io,
            "file_io should be true when FILE SECTION (FD) is present even without FILE-CONTROL"
        );
    }

    #[test]
    fn test_file_io_false_for_no_files() {
        let summary = analyze_file(Path::new("test.cob"), HELLO_WORLD).unwrap();
        assert!(
            !summary.file_io,
            "file_io should be false for programs with no file access"
        );
    }

    // --- FD record_len / block_contains parsing ---

    const WITH_RECORD_CONTAINS: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIXED-READ.
       DATA DIVISION.
       FILE SECTION.
       FD  TRANS-FILE
           RECORD CONTAINS 80 CHARACTERS
           BLOCK CONTAINS 10 RECORDS.
       01  TRANS-REC PIC X(80).
       WORKING-STORAGE SECTION.
       01  WS-EOF PIC 9 VALUE 0.
       PROCEDURE DIVISION.
           STOP RUN.
"#;

    #[test]
    fn test_fd_record_contains_parsed() {
        let program = parse_cobol_source(WITH_RECORD_CONTAINS).unwrap();
        let data = program.data.unwrap();
        assert_eq!(data.file_section.len(), 1);
        let fd = &data.file_section[0];
        assert_eq!(fd.fd_name, "TRANS-FILE");
        assert_eq!(fd.record_len, Some(80));
        assert_eq!(fd.block_contains, Some(10));
    }

    const WITH_MULTI_FD: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. MULTI-FD.
       DATA DIVISION.
       FILE SECTION.
       FD  INPUT-FILE
           RECORD CONTAINS 21 CHARACTERS.
       01  INPUT-RECORD PIC X(21).
       FD  OUTPUT-FILE
           RECORD CONTAINS 80 CHARACTERS.
       01  OUTPUT-RECORD PIC X(80).
       WORKING-STORAGE SECTION.
       01  WS-EOF PIC 9 VALUE 0.
       PROCEDURE DIVISION.
           STOP RUN.
"#;

    #[test]
    fn test_fd_multiple_fds_parsed() {
        let program = parse_cobol_source(WITH_MULTI_FD).unwrap();
        let data = program.data.unwrap();
        assert_eq!(data.file_section.len(), 2);
        assert_eq!(data.file_section[0].fd_name, "INPUT-FILE");
        assert_eq!(data.file_section[0].record_len, Some(21));
        assert_eq!(data.file_section[1].fd_name, "OUTPUT-FILE");
        assert_eq!(data.file_section[1].record_len, Some(80));
    }

    #[test]
    fn test_fd_no_record_contains() {
        // FD without RECORD CONTAINS should have record_len = None
        let program = parse_cobol_source(WITH_FILE_SECTION_ONLY).unwrap();
        let data = program.data.unwrap();
        assert_eq!(data.file_section.len(), 2);
        assert_eq!(data.file_section[0].record_len, None);
        assert_eq!(data.file_section[0].block_contains, None);
    }

    // --- COMPUTE ROUNDED ---

    #[test]
    fn test_compute_rounded_detected() {
        let stmt = parse_compute_statement("COMPUTE WS-RESULT ROUNDED = WS-A / WS-B.");
        if let Statement::Compute(cs) = stmt {
            assert_eq!(cs.target, "WS-RESULT");
            assert!(cs.rounded, "ROUNDED keyword should be detected");
            assert_eq!(cs.expression, "WS-A / WS-B");
        } else {
            panic!("Expected Compute statement");
        }
    }

    #[test]
    fn test_compute_without_rounded() {
        let stmt = parse_compute_statement("COMPUTE WS-RESULT = WS-A + WS-B.");
        if let Statement::Compute(cs) = stmt {
            assert_eq!(cs.target, "WS-RESULT");
            assert!(
                !cs.rounded,
                "rounded should be false without ROUNDED keyword"
            );
            assert_eq!(cs.expression, "WS-A + WS-B");
        } else {
            panic!("Expected Compute statement");
        }
    }

    // --- Fixed-length sequential read/write pattern ---

    /// Verify that a fixed-length COBOL sequential file test can be simulated.
    /// This test validates the expected Rust read_exact pattern for 80-byte records.
    #[test]
    fn test_fixed_length_sequential_read() {
        // Simulate what the Rust codegen should produce for a RECORD CONTAINS 80 CHARACTERS FD:
        // read_exact reads exactly N bytes per record regardless of newlines.
        let record_len: usize = 80;
        let mut data = Vec::new();
        // Write 3 fixed-length records (no newlines - true sequential file)
        for i in 0u8..3 {
            let mut rec = vec![b'A' + i; record_len];
            rec[79] = b'0' + i; // last byte marker
            data.extend_from_slice(&rec);
        }

        let mut cursor = std::io::Cursor::new(&data);
        use std::io::Read;
        let mut records_read = 0usize;
        let mut buf = vec![0u8; record_len];
        while cursor.read_exact(&mut buf).is_ok() {
            records_read += 1;
            // Each record should be exactly record_len bytes
            assert_eq!(buf.len(), record_len);
        }
        assert_eq!(
            records_read, 3,
            "Should read exactly 3 fixed-length records"
        );
    }

    #[test]
    fn test_fixed_length_sequential_write() {
        let record_len: usize = 21; // BATCHUPD-style 21-char record
        use std::io::Write;
        let mut output = Vec::new();
        let records = vec!["12345678901D000000099", "98765432100W000001000"];
        for rec in &records {
            let bytes = rec.as_bytes();
            // Pad or truncate to fixed length
            let mut padded = vec![b' '; record_len];
            let copy_len = bytes.len().min(record_len);
            padded[..copy_len].copy_from_slice(&bytes[..copy_len]);
            output.write_all(&padded).unwrap();
        }
        // Total output must be exactly records * record_len bytes
        assert_eq!(
            output.len(),
            records.len() * record_len,
            "Fixed-length output must have no padding/gaps between records"
        );
    }
}
