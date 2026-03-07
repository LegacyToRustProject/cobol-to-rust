/// Structured representation of a COBOL FD (File Description) entry.
///
/// Parsed from constructs like:
/// ```cobol
/// FD  TRANS-FILE
///     RECORD CONTAINS 80 CHARACTERS
///     BLOCK CONTAINS 10 RECORDS.
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FdDefinition {
    pub name: String,
    /// Fixed record length in bytes, from `RECORD CONTAINS N CHARACTERS`.
    /// `None` if not specified (variable-length or unspecified).
    pub record_len: Option<usize>,
    /// Block factor from `BLOCK CONTAINS N RECORDS`.
    pub block_contains: Option<usize>,
    /// File organization (default: Sequential).
    pub organization: FileOrganization,
}

/// COBOL file organization modes.
#[derive(Debug, Clone, PartialEq)]
pub enum FileOrganization {
    Sequential,
    Indexed,
    Relative,
}

impl FdDefinition {
    /// Returns the Rust `read_exact` snippet for reading one fixed-length record.
    /// Returns `None` if `record_len` is not known.
    pub fn rust_read_exact_snippet(&self) -> Option<String> {
        let n = self.record_len?;
        Some(format!(
            r#"let mut buf = vec![0u8; {n}];
match reader.read_exact(&mut buf) {{
    Ok(()) => {{ /* process buf */ }}
    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
    Err(e) => return Err(e.into()),
}}"#
        ))
    }

    /// Returns the Rust `write_all` snippet for writing one fixed-length record.
    /// Returns `None` if `record_len` is not known.
    pub fn rust_write_all_snippet(&self) -> Option<String> {
        let n = self.record_len?;
        Some(format!(
            r#"let mut record = vec![b' '; {n}];
// Fill record bytes, then:
writer.write_all(&record)?;"#
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fd_definition_default_sequential() {
        let fd = FdDefinition {
            name: "TRANS-FILE".to_string(),
            record_len: Some(80),
            block_contains: Some(10),
            organization: FileOrganization::Sequential,
        };
        assert_eq!(fd.name, "TRANS-FILE");
        assert_eq!(fd.record_len, Some(80));
        assert_eq!(fd.block_contains, Some(10));
        assert_eq!(fd.organization, FileOrganization::Sequential);
    }

    #[test]
    fn test_fd_definition_no_record_len() {
        let fd = FdDefinition {
            name: "VAR-FILE".to_string(),
            record_len: None,
            block_contains: None,
            organization: FileOrganization::Sequential,
        };
        assert!(fd.record_len.is_none());
        assert!(fd.rust_read_exact_snippet().is_none());
        assert!(fd.rust_write_all_snippet().is_none());
    }

    #[test]
    fn test_rust_read_exact_snippet_contains_read_exact() {
        let fd = FdDefinition {
            name: "INPUT-FILE".to_string(),
            record_len: Some(21),
            block_contains: None,
            organization: FileOrganization::Sequential,
        };
        let snippet = fd.rust_read_exact_snippet().unwrap();
        assert!(
            snippet.contains("read_exact"),
            "snippet should use read_exact"
        );
        assert!(
            snippet.contains("21"),
            "snippet should reference record length 21"
        );
    }

    #[test]
    fn test_rust_write_all_snippet_contains_write_all() {
        let fd = FdDefinition {
            name: "OUTPUT-FILE".to_string(),
            record_len: Some(80),
            block_contains: None,
            organization: FileOrganization::Sequential,
        };
        let snippet = fd.rust_write_all_snippet().unwrap();
        assert!(
            snippet.contains("write_all"),
            "snippet should use write_all"
        );
        assert!(
            snippet.contains("80"),
            "snippet should reference record length 80"
        );
    }

    #[test]
    fn test_fd_indexed_organization() {
        let fd = FdDefinition {
            name: "CUSTOMER-FILE".to_string(),
            record_len: Some(200),
            block_contains: None,
            organization: FileOrganization::Indexed,
        };
        assert_eq!(fd.organization, FileOrganization::Indexed);
    }
}
