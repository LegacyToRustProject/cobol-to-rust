/// COBOL EXEC SQL ... END-EXEC statement parser.
///
/// Handles embedded SQL in COBOL programs.  Common patterns:
///
/// ```text
/// EXEC SQL
///     SELECT ACCT_BAL INTO :WS-BALANCE FROM ACCOUNTS WHERE ACCT_ID = :WS-ID
/// END-EXEC.
///
/// EXEC SQL
///     INSERT INTO LEDGER (ACCT_ID, AMOUNT) VALUES (:WS-ID, :WS-AMT)
/// END-EXEC.
/// ```
use crate::types::{
    ExecSqlStatement, SqlDeclareCursor, SqlDelete, SqlFetch, SqlInsert, SqlSelect, SqlUpdate,
    SqlWhenever, SqlWheneverAction, SqlWheneverCondition,
};

/// Extract and parse an EXEC SQL block from COBOL source lines.
///
/// `lines` should be the full list of processed (stripped) COBOL lines.
/// Returns a list of `(line_index_after_end_exec, ExecSqlStatement)` pairs.
pub fn extract_exec_sql_blocks(lines: &[&str]) -> Vec<(usize, ExecSqlStatement)> {
    let mut results = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let upper = lines[i].to_uppercase();
        let trimmed = upper.trim();

        if trimmed.starts_with("EXEC SQL") {
            // Collect all lines until END-EXEC
            let mut block_lines: Vec<&str> = Vec::new();

            // The EXEC SQL line itself may contain the first part of the statement
            let rest = lines[i][lines[i].to_uppercase().find("EXEC SQL").unwrap() + 8..]
                .trim()
                .to_string();
            if !rest.is_empty() {
                block_lines.push(lines[i]); // we'll re-parse from the block
            }

            let start = i;
            i += 1;

            // Accumulate until END-EXEC (or EOF)
            while i < lines.len() {
                let up = lines[i].to_uppercase();
                if up.trim().contains("END-EXEC") {
                    break;
                }
                block_lines.push(lines[i]);
                i += 1;
            }

            // Build full SQL text from all collected lines
            let mut full_text = String::new();
            // Include content from the EXEC SQL line after "EXEC SQL"
            let exec_line_upper = lines[start].to_uppercase();
            let after_exec_sql = &lines[start][exec_line_upper.find("EXEC SQL").unwrap() + 8..]
                .trim()
                .to_string();
            if !after_exec_sql.is_empty() {
                full_text.push_str(after_exec_sql);
                full_text.push(' ');
            }
            for l in &block_lines {
                let up = l.to_uppercase();
                // skip the EXEC SQL line itself (already handled)
                if up.trim().starts_with("EXEC SQL") {
                    continue;
                }
                full_text.push_str(l.trim());
                full_text.push(' ');
            }

            // Strip trailing END-EXEC (may appear inline on same line)
            let sql_text_raw = full_text.trim().trim_end_matches('.').trim();
            let sql_text = if let Some(end_pos) = sql_text_raw.to_uppercase().find("END-EXEC") {
                sql_text_raw[..end_pos].trim().to_string()
            } else {
                sql_text_raw.to_string()
            };
            let stmt = parse_exec_sql(&sql_text);
            results.push((i + 1, stmt));
        }

        i += 1;
    }

    results
}

/// Parse an EXEC SQL statement body (without the EXEC SQL / END-EXEC wrapper).
pub fn parse_exec_sql(sql: &str) -> ExecSqlStatement {
    let upper = sql.to_uppercase();
    let trimmed = upper.trim();

    if trimmed.starts_with("SELECT") {
        parse_select(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else if trimmed.starts_with("INSERT") {
        parse_insert(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else if trimmed.starts_with("UPDATE") {
        parse_update(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else if trimmed.starts_with("DELETE") {
        parse_delete(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else if trimmed.starts_with("DECLARE") {
        parse_declare_cursor(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else if trimmed.starts_with("OPEN") {
        parse_open_cursor(sql)
    } else if trimmed.starts_with("FETCH") {
        parse_fetch(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else if trimmed.starts_with("CLOSE") {
        parse_close_cursor(sql)
    } else if trimmed == "COMMIT" || trimmed.starts_with("COMMIT ") {
        ExecSqlStatement::Commit
    } else if trimmed == "ROLLBACK" || trimmed.starts_with("ROLLBACK ") {
        ExecSqlStatement::Rollback
    } else if trimmed.starts_with("WHENEVER") {
        parse_whenever(sql).unwrap_or(ExecSqlStatement::Unknown(sql.to_string()))
    } else {
        ExecSqlStatement::Unknown(sql.to_string())
    }
}

/// Parse: SELECT col [, col...] INTO :var [, :var...] FROM table [WHERE cond]
fn parse_select(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();

    // Find INTO position
    let into_pos = upper.find(" INTO ")?;
    let from_pos = upper.find(" FROM ")?;

    // Columns: between SELECT and INTO
    let cols_str = &sql[7..into_pos]; // skip "SELECT "
    let columns: Vec<String> = cols_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // INTO vars: between INTO and FROM
    let into_str = &sql[into_pos + 6..from_pos]; // skip " INTO "
    let into_vars: Vec<String> = into_str
        .split(',')
        .map(|s| s.trim().trim_start_matches(':').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Table + optional WHERE
    let after_from = &sql[from_pos + 6..]; // skip " FROM "
    let where_pos = after_from.to_uppercase().find(" WHERE ");
    let (table, where_clause) = if let Some(wp) = where_pos {
        (
            after_from[..wp].trim().to_string(),
            Some(after_from[wp + 7..].trim().to_string()),
        )
    } else {
        (after_from.trim().to_string(), None)
    };

    Some(ExecSqlStatement::Select(SqlSelect {
        columns,
        into_vars,
        table,
        where_clause,
    }))
}

/// Parse: INSERT INTO table (col1, col2) VALUES (:var1, :var2)
fn parse_insert(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();

    // Find table name: INSERT INTO <table>
    let into_pos = upper.find(" INTO ")? + 6;
    let paren_pos = upper.find('(')?;
    let table = sql[into_pos..paren_pos].trim().to_string();

    // Columns: first parenthesized list
    let close_paren = upper.find(')')?;
    let cols_str = &sql[paren_pos + 1..close_paren];
    let columns: Vec<String> = cols_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // VALUES: second parenthesized list
    let values_pos = upper.find("VALUES")?;
    let val_open = upper[values_pos..].find('(')? + values_pos;
    let val_close = upper[val_open..].find(')')? + val_open;
    let vals_str = &sql[val_open + 1..val_close];
    let values: Vec<String> = vals_str
        .split(',')
        .map(|s| s.trim().trim_start_matches(':').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Some(ExecSqlStatement::Insert(SqlInsert {
        table,
        columns,
        values,
    }))
}

/// Parse: UPDATE table SET col1 = :var1, col2 = :var2 [WHERE cond]
fn parse_update(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();

    let set_pos = upper.find(" SET ")?;
    let table = sql[7..set_pos].trim().to_string(); // skip "UPDATE "

    let after_set = &sql[set_pos + 5..]; // skip " SET "
    let after_set_upper = after_set.to_uppercase();
    let where_pos = after_set_upper.find(" WHERE ");

    let (set_str, where_clause) = if let Some(wp) = where_pos {
        (
            &after_set[..wp],
            Some(after_set[wp + 7..].trim().to_string()),
        )
    } else {
        (after_set, None)
    };

    let set_clauses: Vec<(String, String)> = set_str
        .split(',')
        .filter_map(|clause| {
            let eq_pos = clause.find('=')?;
            let col = clause[..eq_pos].trim().to_string();
            let val = clause[eq_pos + 1..]
                .trim()
                .trim_start_matches(':')
                .to_string();
            Some((col, val))
        })
        .collect();

    Some(ExecSqlStatement::Update(SqlUpdate {
        table,
        set_clauses,
        where_clause,
    }))
}

/// Parse: DELETE FROM table [WHERE cond]
fn parse_delete(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();
    let from_pos = upper.find(" FROM ")? + 6;
    let after_from = &sql[from_pos..];
    let after_from_upper = after_from.to_uppercase();
    let where_pos = after_from_upper.find(" WHERE ");

    let (table, where_clause) = if let Some(wp) = where_pos {
        (
            after_from[..wp].trim().to_string(),
            Some(after_from[wp + 7..].trim().to_string()),
        )
    } else {
        (after_from.trim().to_string(), None)
    };

    Some(ExecSqlStatement::Delete(SqlDelete {
        table,
        where_clause,
    }))
}

/// Parse: DECLARE cursor CURSOR FOR SELECT ...
fn parse_declare_cursor(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();
    let cursor_for_pos = upper.find(" CURSOR FOR ")?;
    let cursor_name = sql[8..cursor_for_pos].trim().to_string(); // skip "DECLARE "
    let select_sql = &sql[cursor_for_pos + 12..]; // skip " CURSOR FOR "
    if let ExecSqlStatement::Select(sel) = parse_select(select_sql)? {
        Some(ExecSqlStatement::DeclareCursor(SqlDeclareCursor {
            cursor_name,
            select: sel,
        }))
    } else {
        None
    }
}

/// Parse: OPEN cursor
fn parse_open_cursor(sql: &str) -> ExecSqlStatement {
    let cursor_name = sql[5..].trim().to_string(); // skip "OPEN "
    ExecSqlStatement::OpenCursor(cursor_name)
}

/// Parse: FETCH cursor INTO :var1, :var2
fn parse_fetch(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();
    let into_pos = upper.find(" INTO ")?;
    let cursor_name = sql[6..into_pos].trim().to_string(); // skip "FETCH "
    let into_vars: Vec<String> = sql[into_pos + 6..]
        .split(',')
        .map(|s| s.trim().trim_start_matches(':').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(ExecSqlStatement::FetchCursor(SqlFetch {
        cursor_name,
        into_vars,
    }))
}

/// Parse: CLOSE cursor
fn parse_close_cursor(sql: &str) -> ExecSqlStatement {
    let cursor_name = sql[6..].trim().to_string(); // skip "CLOSE "
    ExecSqlStatement::CloseCursor(cursor_name)
}

/// Parse: WHENEVER {NOT FOUND|SQLERROR|SQLWARNING} {CONTINUE|GOTO label|STOP}
fn parse_whenever(sql: &str) -> Option<ExecSqlStatement> {
    let upper = sql.to_uppercase();

    let condition = if upper.contains("NOT FOUND") {
        SqlWheneverCondition::NotFound
    } else if upper.contains("SQLERROR") {
        SqlWheneverCondition::SqlError
    } else if upper.contains("SQLWARNING") {
        SqlWheneverCondition::SqlWarning
    } else {
        return None;
    };

    let action = if upper.contains("CONTINUE") {
        SqlWheneverAction::Continue
    } else if upper.contains("STOP") {
        SqlWheneverAction::Stop
    } else if let Some(goto_pos) = upper.find("GOTO") {
        let label = sql[goto_pos + 4..].trim().to_string();
        SqlWheneverAction::GoTo(label)
    } else if let Some(goto_pos) = upper.find("GO TO") {
        let label = sql[goto_pos + 5..].trim().to_string();
        SqlWheneverAction::GoTo(label)
    } else {
        return None;
    };

    Some(ExecSqlStatement::Whenever(SqlWhenever {
        condition,
        action,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SqlWheneverAction, SqlWheneverCondition};

    // --- Pattern 1: Simple SELECT INTO ---
    #[test]
    fn test_select_single_column_into() {
        let sql = "SELECT ACCT_BAL INTO :WS-BALANCE FROM ACCOUNTS WHERE ACCT_ID = :WS-ID";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Select(sel) = stmt {
            assert_eq!(sel.columns, vec!["ACCT_BAL"]);
            assert_eq!(sel.into_vars, vec!["WS-BALANCE"]);
            assert_eq!(sel.table, "ACCOUNTS");
            assert_eq!(sel.where_clause, Some("ACCT_ID = :WS-ID".to_string()));
        } else {
            panic!("Expected Select");
        }
    }

    // --- Pattern 2: Multi-column SELECT INTO ---
    #[test]
    fn test_select_multi_column_into() {
        let sql =
            "SELECT FIRST_NAME, LAST_NAME, SALARY INTO :WS-FNAME, :WS-LNAME, :WS-SAL FROM EMPLOYEES WHERE EMP_ID = :WS-EMP-ID";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Select(sel) = stmt {
            assert_eq!(sel.columns.len(), 3);
            assert_eq!(sel.columns[0], "FIRST_NAME");
            assert_eq!(sel.into_vars.len(), 3);
            assert_eq!(sel.into_vars[0], "WS-FNAME");
            assert_eq!(sel.into_vars[2], "WS-SAL");
        } else {
            panic!("Expected Select");
        }
    }

    // --- Pattern 3: SELECT without WHERE ---
    #[test]
    fn test_select_no_where() {
        let sql = "SELECT COUNT(*) INTO :WS-CNT FROM ORDERS";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Select(sel) = stmt {
            assert_eq!(sel.columns, vec!["COUNT(*)"]);
            assert_eq!(sel.into_vars, vec!["WS-CNT"]);
            assert!(sel.where_clause.is_none());
        } else {
            panic!("Expected Select");
        }
    }

    // --- Pattern 4: INSERT ---
    #[test]
    fn test_insert_with_columns_and_values() {
        let sql =
            "INSERT INTO LEDGER (ACCT_ID, TRANS_DATE, AMOUNT) VALUES (:WS-ID, :WS-DATE, :WS-AMT)";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Insert(ins) = stmt {
            assert_eq!(ins.table, "LEDGER");
            assert_eq!(ins.columns, vec!["ACCT_ID", "TRANS_DATE", "AMOUNT"]);
            assert_eq!(ins.values, vec!["WS-ID", "WS-DATE", "WS-AMT"]);
        } else {
            panic!("Expected Insert");
        }
    }

    // --- Pattern 5: UPDATE with WHERE ---
    #[test]
    fn test_update_single_column() {
        let sql = "UPDATE ACCOUNTS SET BALANCE = :WS-NEW-BAL WHERE ACCT_ID = :WS-ID";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Update(upd) = stmt {
            assert_eq!(upd.table, "ACCOUNTS");
            assert_eq!(upd.set_clauses.len(), 1);
            assert_eq!(upd.set_clauses[0].0, "BALANCE");
            assert_eq!(upd.set_clauses[0].1, "WS-NEW-BAL");
            assert!(upd.where_clause.is_some());
        } else {
            panic!("Expected Update");
        }
    }

    // --- Pattern 6: DELETE with WHERE ---
    #[test]
    fn test_delete_with_where() {
        let sql = "DELETE FROM TEMP_ORDERS WHERE ORDER_DATE < :WS-CUTOFF-DATE";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Delete(del) = stmt {
            assert_eq!(del.table, "TEMP_ORDERS");
            assert_eq!(
                del.where_clause,
                Some("ORDER_DATE < :WS-CUTOFF-DATE".to_string())
            );
        } else {
            panic!("Expected Delete");
        }
    }

    // --- Pattern 7: DELETE without WHERE ---
    #[test]
    fn test_delete_no_where() {
        let sql = "DELETE FROM WORK_TABLE";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Delete(del) = stmt {
            assert_eq!(del.table, "WORK_TABLE");
            assert!(del.where_clause.is_none());
        } else {
            panic!("Expected Delete");
        }
    }

    // --- Pattern 8: DECLARE CURSOR FOR SELECT ---
    #[test]
    fn test_declare_cursor_for_select() {
        let sql = "DECLARE ACCT-CUR CURSOR FOR SELECT ACCT_ID, BALANCE INTO :WS-ID, :WS-BAL FROM ACCOUNTS WHERE STATUS = :WS-STATUS";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::DeclareCursor(dc) = stmt {
            assert_eq!(dc.cursor_name, "ACCT-CUR");
            assert_eq!(dc.select.table, "ACCOUNTS");
            assert_eq!(dc.select.columns.len(), 2);
        } else {
            panic!("Expected DeclareCursor");
        }
    }

    // --- Pattern 9: OPEN / FETCH / CLOSE cursor ---
    #[test]
    fn test_open_cursor() {
        let stmt = parse_exec_sql("OPEN ACCT-CUR");
        assert!(matches!(stmt, ExecSqlStatement::OpenCursor(n) if n == "ACCT-CUR"));
    }

    #[test]
    fn test_fetch_cursor_into() {
        let sql = "FETCH ACCT-CUR INTO :WS-ACCT-ID, :WS-BAL";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::FetchCursor(fetch) = stmt {
            assert_eq!(fetch.cursor_name, "ACCT-CUR");
            assert_eq!(fetch.into_vars, vec!["WS-ACCT-ID", "WS-BAL"]);
        } else {
            panic!("Expected FetchCursor");
        }
    }

    #[test]
    fn test_close_cursor() {
        let stmt = parse_exec_sql("CLOSE ACCT-CUR");
        assert!(matches!(stmt, ExecSqlStatement::CloseCursor(n) if n == "ACCT-CUR"));
    }

    // --- Pattern 10: COMMIT and ROLLBACK ---
    #[test]
    fn test_commit() {
        let stmt = parse_exec_sql("COMMIT");
        assert!(matches!(stmt, ExecSqlStatement::Commit));
    }

    #[test]
    fn test_rollback() {
        let stmt = parse_exec_sql("ROLLBACK");
        assert!(matches!(stmt, ExecSqlStatement::Rollback));
    }

    // --- Pattern 11: WHENEVER conditions ---
    #[test]
    fn test_whenever_not_found_continue() {
        let stmt = parse_exec_sql("WHENEVER NOT FOUND CONTINUE");
        if let ExecSqlStatement::Whenever(w) = stmt {
            assert_eq!(w.condition, SqlWheneverCondition::NotFound);
            assert_eq!(w.action, SqlWheneverAction::Continue);
        } else {
            panic!("Expected Whenever");
        }
    }

    #[test]
    fn test_whenever_sqlerror_goto() {
        let stmt = parse_exec_sql("WHENEVER SQLERROR GOTO ERROR-HANDLER");
        if let ExecSqlStatement::Whenever(w) = stmt {
            assert_eq!(w.condition, SqlWheneverCondition::SqlError);
            assert!(
                matches!(w.action, SqlWheneverAction::GoTo(ref l) if l.contains("ERROR-HANDLER"))
            );
        } else {
            panic!("Expected Whenever");
        }
    }

    // --- Pattern 12: UPDATE multi-column SET ---
    #[test]
    fn test_update_multi_column() {
        let sql = "UPDATE EMP SET SALARY = :WS-NEW-SAL, DEPT = :WS-DEPT WHERE EMP_ID = :WS-EMP-ID";
        let stmt = parse_exec_sql(sql);
        if let ExecSqlStatement::Update(upd) = stmt {
            assert_eq!(upd.set_clauses.len(), 2);
            assert_eq!(upd.set_clauses[0].0.trim(), "SALARY");
            assert_eq!(upd.set_clauses[1].0.trim(), "DEPT");
        } else {
            panic!("Expected Update");
        }
    }

    // --- extract_exec_sql_blocks integration test ---
    #[test]
    fn test_extract_exec_sql_blocks_from_cobol_lines() {
        let source = vec![
            "       EXEC SQL",
            "           SELECT BAL INTO :WS-BAL FROM ACCTS WHERE ID = :WS-ID",
            "       END-EXEC.",
            "       MOVE WS-BAL TO WS-DISPLAY.",
            "       EXEC SQL COMMIT END-EXEC.",
        ];
        let results = extract_exec_sql_blocks(&source);
        assert_eq!(results.len(), 2, "should find 2 EXEC SQL blocks");
        assert!(matches!(results[0].1, ExecSqlStatement::Select(_)));
        assert!(matches!(results[1].1, ExecSqlStatement::Commit));
    }
}
