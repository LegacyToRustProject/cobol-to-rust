/// sqlx::query! macro code generator for COBOL EXEC SQL statements.
///
/// Maps COBOL embedded SQL patterns to Rust sqlx async query code:
/// - SELECT INTO → `sqlx::query_as!` or `sqlx::query_scalar!`
/// - INSERT/UPDATE/DELETE → `sqlx::query!`
/// - Cursor operations → `sqlx::query!(...).fetch()`
/// - COMMIT/ROLLBACK → `tx.commit()` / `tx.rollback()`
use cobol_parser::types::{ExecSqlStatement, SqlSelect, SqlWheneverAction, SqlWheneverCondition};

/// The generated Rust code for a single EXEC SQL statement.
#[derive(Debug, Clone)]
pub struct SqlxCode {
    /// The generated Rust expression/statement
    pub code: String,
    /// Whether this requires `async` context
    pub is_async: bool,
    /// Whether the generated code uses a struct (needs struct definition)
    pub needs_row_struct: bool,
    /// Optional row struct name
    pub row_struct_name: Option<String>,
}

/// Generate Rust sqlx code for an EXEC SQL statement.
pub fn generate_sqlx(stmt: &ExecSqlStatement) -> SqlxCode {
    match stmt {
        ExecSqlStatement::Select(sel) => generate_select(sel),
        ExecSqlStatement::Insert(ins) => {
            let params: Vec<String> = ins.values.iter().map(|v| to_rust_var(v)).collect();
            let placeholders: Vec<String> =
                (1..=ins.values.len()).map(|i| format!("${}", i)).collect();
            let cols = ins.columns.join(", ");
            let vals = placeholders.join(", ");
            let code = format!(
                "sqlx::query!(\n    \"INSERT INTO {} ({}) VALUES ({})\",\n    {}\n).execute(&pool).await?;",
                ins.table,
                cols,
                vals,
                params.join(",\n    ")
            );
            SqlxCode {
                code,
                is_async: true,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::Update(upd) => {
            let set_parts: Vec<String> = upd
                .set_clauses
                .iter()
                .enumerate()
                .map(|(i, (col, _))| format!("{} = ${}", col.trim(), i + 1))
                .collect();
            let params: Vec<String> = upd
                .set_clauses
                .iter()
                .map(|(_, var)| to_rust_var(var))
                .collect();
            let where_clause = upd.where_clause.as_deref().unwrap_or("1=1");
            let where_param_offset = upd.set_clauses.len() + 1;
            let where_sql = replace_cobol_params(where_clause, where_param_offset);
            let where_vars: Vec<String> = extract_cobol_vars(where_clause)
                .iter()
                .map(|v| to_rust_var(v))
                .collect();
            let mut all_params = params;
            all_params.extend(where_vars);
            let code = format!(
                "sqlx::query!(\n    \"UPDATE {} SET {} WHERE {}\",\n    {}\n).execute(&pool).await?;",
                upd.table,
                set_parts.join(", "),
                where_sql,
                all_params.join(",\n    ")
            );
            SqlxCode {
                code,
                is_async: true,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::Delete(del) => {
            let where_clause = del.where_clause.as_deref().unwrap_or("1=1");
            let where_sql = replace_cobol_params(where_clause, 1);
            let where_vars: Vec<String> = extract_cobol_vars(where_clause)
                .iter()
                .map(|v| to_rust_var(v))
                .collect();
            let params_str = if where_vars.is_empty() {
                String::new()
            } else {
                format!(",\n    {}", where_vars.join(",\n    "))
            };
            let code = format!(
                "sqlx::query!(\"DELETE FROM {} WHERE {}\"{}).execute(&pool).await?;",
                del.table, where_sql, params_str
            );
            SqlxCode {
                code,
                is_async: true,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::DeclareCursor(dc) => {
            // Cursor declaration → sqlx query builder stored as a variable
            let sel = &dc.select;
            let cols = sel.columns.join(", ");
            let where_sql = sel
                .where_clause
                .as_deref()
                .map(|w| format!(" WHERE {}", replace_cobol_params(w, 1)))
                .unwrap_or_default();
            let where_vars: Vec<String> = sel
                .where_clause
                .as_deref()
                .map(|w| {
                    extract_cobol_vars(w)
                        .iter()
                        .map(|v| to_rust_var(v))
                        .collect()
                })
                .unwrap_or_default();
            let cursor_var = to_rust_var(&dc.cursor_name);
            let params_str = if where_vars.is_empty() {
                String::new()
            } else {
                format!(",\n    {}", where_vars.join(",\n    "))
            };
            let code = format!(
                "let mut {} = sqlx::query!(\"SELECT {} FROM {}{}\"{}).fetch(&pool);",
                cursor_var, cols, sel.table, where_sql, params_str
            );
            SqlxCode {
                code,
                is_async: true,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::OpenCursor(_) => SqlxCode {
            code: "// OPEN cursor: cursor was already prepared via sqlx query builder".to_string(),
            is_async: false,
            needs_row_struct: false,
            row_struct_name: None,
        },
        ExecSqlStatement::FetchCursor(fetch) => {
            let cursor_var = to_rust_var(&fetch.cursor_name);
            let bindings: Vec<String> = fetch
                .into_vars
                .iter()
                .enumerate()
                .map(|(i, v)| format!("let {} = row.{};", to_rust_var(v), i))
                .collect();
            let code = format!(
                "if let Some(row) = {}.try_next().await? {{\n    {}\n}}",
                cursor_var,
                bindings.join("\n    ")
            );
            SqlxCode {
                code,
                is_async: true,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::CloseCursor(name) => {
            let cursor_var = to_rust_var(name);
            SqlxCode {
                code: format!("drop({});", cursor_var),
                is_async: false,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::Commit => SqlxCode {
            code: "tx.commit().await?;".to_string(),
            is_async: true,
            needs_row_struct: false,
            row_struct_name: None,
        },
        ExecSqlStatement::Rollback => SqlxCode {
            code: "tx.rollback().await?;".to_string(),
            is_async: true,
            needs_row_struct: false,
            row_struct_name: None,
        },
        ExecSqlStatement::Whenever(w) => {
            let condition = match w.condition {
                SqlWheneverCondition::NotFound => "sqlx::Error::RowNotFound",
                SqlWheneverCondition::SqlError => "sqlx::Error",
                SqlWheneverCondition::SqlWarning => "// SQLWARNING",
            };
            let action = match &w.action {
                SqlWheneverAction::Continue => "{ /* continue */ }",
                SqlWheneverAction::Stop => "{ std::process::exit(1); }",
                SqlWheneverAction::GoTo(label) => {
                    return SqlxCode {
                        code: format!(
                            "// WHENEVER {} GOTO {} → handle in match/if-let",
                            condition,
                            to_rust_var(label)
                        ),
                        is_async: false,
                        needs_row_struct: false,
                        row_struct_name: None,
                    };
                }
            };
            SqlxCode {
                code: format!(
                    "// WHENEVER {}: if err matches {} {}",
                    condition, condition, action
                ),
                is_async: false,
                needs_row_struct: false,
                row_struct_name: None,
            }
        }
        ExecSqlStatement::Unknown(raw) => SqlxCode {
            code: format!("// EXEC SQL (unrecognized): {}", raw),
            is_async: false,
            needs_row_struct: false,
            row_struct_name: None,
        },
    }
}

fn generate_select(sel: &SqlSelect) -> SqlxCode {
    let cols = sel.columns.join(", ");
    let where_sql = sel
        .where_clause
        .as_deref()
        .map(|w| format!(" WHERE {}", replace_cobol_params(w, 1)))
        .unwrap_or_default();
    let where_vars: Vec<String> = sel
        .where_clause
        .as_deref()
        .map(|w| {
            extract_cobol_vars(w)
                .iter()
                .map(|v| to_rust_var(v))
                .collect()
        })
        .unwrap_or_default();

    let params_str = if where_vars.is_empty() {
        String::new()
    } else {
        format!(",\n    {}", where_vars.join(",\n    "))
    };

    let bindings: Vec<String> = sel
        .into_vars
        .iter()
        .zip(sel.columns.iter())
        .map(|(var, col)| {
            let rust_var = to_rust_var(var);
            let col_lower = col
                .to_lowercase()
                .replace('(', "_")
                .replace(')', "")
                .replace('*', "star");
            format!("let {} = row.{};", rust_var, col_lower)
        })
        .collect();

    if sel.into_vars.len() == 1 {
        // Single column: use query_scalar!
        let code = format!(
            "let {}: _ = sqlx::query_scalar!(\"SELECT {} FROM {}{}\"{}).fetch_one(&pool).await?;\n{}",
            to_rust_var(&sel.into_vars[0]),
            cols,
            sel.table,
            where_sql,
            params_str,
            bindings[0]
        );
        SqlxCode {
            code,
            is_async: true,
            needs_row_struct: false,
            row_struct_name: None,
        }
    } else {
        // Multi-column: use query!
        let code = format!(
            "let row = sqlx::query!(\"SELECT {} FROM {}{}\"{}).fetch_one(&pool).await?;\n{}",
            cols,
            sel.table,
            where_sql,
            params_str,
            bindings.join("\n")
        );
        SqlxCode {
            code,
            is_async: true,
            needs_row_struct: false,
            row_struct_name: None,
        }
    }
}

/// Replace COBOL :VARIABLE references with sqlx $N positional parameters.
/// e.g. "ACCT_ID = :WS-ID" → "ACCT_ID = $1"
pub fn replace_cobol_params(sql: &str, start_at: usize) -> String {
    let mut result = sql.to_string();
    let mut counter = start_at;
    // Find :IDENTIFIER patterns and replace with $N
    let re = regex::Regex::new(r":([A-Z][A-Z0-9_-]*)").unwrap();
    let upper = sql.to_uppercase();
    let replaced = re.replace_all(&upper, |_: &regex::Captures| {
        let placeholder = format!("${}", counter);
        counter += 1;
        placeholder
    });
    result = replaced.into_owned();
    result
}

/// Extract COBOL :VARIABLE names from a SQL fragment (preserving original case).
pub fn extract_cobol_vars(sql: &str) -> Vec<String> {
    let re = regex::Regex::new(r":([A-Za-z][A-Za-z0-9_-]*)").unwrap();
    re.captures_iter(sql)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// Convert a COBOL hyphenated name to Rust snake_case.
pub fn to_rust_var(name: &str) -> String {
    name.replace('-', "_").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobol_parser::exec_sql::parse_exec_sql;
    use cobol_parser::types::ExecSqlStatement;

    #[test]
    fn test_select_single_col_generates_query_scalar() {
        let stmt = parse_exec_sql("SELECT BALANCE INTO :WS-BAL FROM ACCOUNTS WHERE ID = :WS-ID");
        let code = generate_sqlx(&stmt);
        assert!(
            code.code.contains("query_scalar!"),
            "single-col SELECT should use query_scalar!"
        );
        assert!(code.is_async, "SELECT should be async");
        assert!(
            code.code.contains("ws_bal"),
            "should contain snake_case variable name"
        );
    }

    #[test]
    fn test_select_multi_col_generates_query() {
        let stmt = parse_exec_sql(
            "SELECT FNAME, LNAME INTO :WS-FNAME, :WS-LNAME FROM EMPLOYEES WHERE EMP_ID = :WS-ID",
        );
        let code = generate_sqlx(&stmt);
        assert!(
            code.code.contains("query!"),
            "multi-col SELECT should use query!"
        );
        assert!(!code.code.contains("query_scalar!"));
    }

    #[test]
    fn test_insert_generates_query() {
        let stmt = parse_exec_sql("INSERT INTO LEDGER (ACCT_ID, AMOUNT) VALUES (:WS-ID, :WS-AMT)");
        let code = generate_sqlx(&stmt);
        assert!(code.code.contains("query!"), "INSERT should use query!");
        assert!(code.code.contains("execute"), "INSERT should call execute");
        assert!(code.code.contains("INSERT INTO LEDGER"));
    }

    #[test]
    fn test_update_generates_query() {
        let stmt =
            parse_exec_sql("UPDATE ACCOUNTS SET BALANCE = :WS-NEW-BAL WHERE ACCT_ID = :WS-ID");
        let code = generate_sqlx(&stmt);
        assert!(code.code.contains("query!"), "UPDATE should use query!");
        assert!(code.code.contains("UPDATE ACCOUNTS SET"));
    }

    #[test]
    fn test_delete_generates_query() {
        let stmt = parse_exec_sql("DELETE FROM TEMP_ORDERS WHERE ORDER_DATE < :WS-DATE");
        let code = generate_sqlx(&stmt);
        assert!(code.code.contains("DELETE FROM TEMP_ORDERS"));
        assert!(code.code.contains("execute"));
    }

    #[test]
    fn test_declare_cursor_generates_fetch() {
        let stmt = parse_exec_sql(
            "DECLARE C1 CURSOR FOR SELECT ID, BAL INTO :WS-ID, :WS-BAL FROM ACCTS WHERE STATUS = :WS-ST",
        );
        let code = generate_sqlx(&stmt);
        assert!(code.code.contains("fetch"), "cursor should use fetch");
        assert!(code.code.contains("c1"), "cursor var should be snake_case");
    }

    #[test]
    fn test_commit_generates_tx_commit() {
        let code = generate_sqlx(&ExecSqlStatement::Commit);
        assert_eq!(code.code, "tx.commit().await?;");
        assert!(code.is_async);
    }

    #[test]
    fn test_rollback_generates_tx_rollback() {
        let code = generate_sqlx(&ExecSqlStatement::Rollback);
        assert_eq!(code.code, "tx.rollback().await?;");
        assert!(code.is_async);
    }

    #[test]
    fn test_replace_cobol_params() {
        let result = replace_cobol_params("ACCT_ID = :WS-ID AND DATE > :WS-DATE", 1);
        assert!(result.contains("$1"), "first param should be $1");
        assert!(result.contains("$2"), "second param should be $2");
        assert!(!result.contains(":WS"), "no COBOL vars should remain");
    }

    #[test]
    fn test_extract_cobol_vars() {
        let vars = extract_cobol_vars("ACCT_ID = :WS-ID AND DATE > :WS-DATE");
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0], "WS-ID");
        assert_eq!(vars[1], "WS-DATE");
    }

    #[test]
    fn test_to_rust_var() {
        assert_eq!(to_rust_var("WS-BALANCE"), "ws_balance");
        assert_eq!(to_rust_var("TR-ACCT-ID"), "tr_acct_id");
    }

    #[test]
    fn test_close_cursor_generates_drop() {
        let stmt = parse_exec_sql("CLOSE ACCT-CUR");
        let code = generate_sqlx(&stmt);
        assert!(code.code.contains("drop("), "CLOSE should generate drop()");
        assert!(code.code.contains("acct_cur"), "cursor name in snake_case");
    }

    #[test]
    fn test_whenever_generates_comment() {
        let stmt = parse_exec_sql("WHENEVER NOT FOUND CONTINUE");
        let code = generate_sqlx(&stmt);
        assert!(
            code.code.starts_with("//"),
            "WHENEVER should generate a comment"
        );
    }

    #[test]
    fn test_open_cursor_generates_comment() {
        let stmt = parse_exec_sql("OPEN MY-CURSOR");
        let code = generate_sqlx(&stmt);
        assert!(
            code.code.starts_with("//"),
            "OPEN cursor should generate a comment"
        );
    }
}
