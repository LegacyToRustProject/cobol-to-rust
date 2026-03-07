use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;

/// Resolves COPY statements by finding and inlining copybook files.
pub struct CopybookResolver {
    /// Directories to search for copybook files.
    search_paths: Vec<PathBuf>,
    /// Cache of resolved copybooks.
    cache: HashMap<String, String>,
}

impl CopybookResolver {
    pub fn new(search_paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths,
            cache: HashMap::new(),
        }
    }

    /// Resolve all COPY statements in the source, replacing them with copybook content.
    pub fn resolve(&mut self, source: &str) -> Result<String> {
        let copy_re = Regex::new(r"(?i)^\s*COPY\s+([\w-]+)\s*\.\s*$").unwrap();
        let mut result = String::new();

        for line in source.lines() {
            if let Some(cap) = copy_re.captures(line) {
                let copybook_name = &cap[1];
                let content = self.load_copybook(copybook_name)?;
                result.push_str(&content);
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }

        Ok(result)
    }

    /// Load a copybook file, using cache if available.
    fn load_copybook(&mut self, name: &str) -> Result<String> {
        if let Some(cached) = self.cache.get(name) {
            return Ok(cached.clone());
        }

        let content = self.find_and_read_copybook(name)?;
        self.cache.insert(name.to_string(), content.clone());
        Ok(content)
    }

    /// Search for a copybook file in the search paths.
    fn find_and_read_copybook(&self, name: &str) -> Result<String> {
        let extensions = ["cpy", "CPY", "cbl", "CBL", "copy", "COPY", ""];
        let name_upper = name.to_uppercase();
        let name_lower = name.to_lowercase();

        for dir in &self.search_paths {
            for ext in &extensions {
                let candidates = if ext.is_empty() {
                    vec![dir.join(name), dir.join(&name_upper), dir.join(&name_lower)]
                } else {
                    vec![
                        dir.join(format!("{name}.{ext}")),
                        dir.join(format!("{name_upper}.{ext}")),
                        dir.join(format!("{name_lower}.{ext}")),
                    ]
                };

                for path in candidates {
                    if path.exists() {
                        return std::fs::read_to_string(&path).with_context(|| {
                            format!("Failed to read copybook: {}", path.display())
                        });
                    }
                }
            }
        }

        anyhow::bail!(
            "Copybook '{}' not found in search paths: {:?}",
            name,
            self.search_paths
        )
    }

    /// List all COPY references found in the source without resolving them.
    pub fn find_copy_references(source: &str) -> Vec<String> {
        let copy_re = Regex::new(r"(?i)^\s*COPY\s+([\w-]+)\s*\.\s*$").unwrap();
        source
            .lines()
            .filter_map(|line| copy_re.captures(line).map(|cap| cap[1].to_string()))
            .collect()
    }
}

/// Find all copybook files in a directory.
pub fn find_copybook_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut copybooks = Vec::new();
    if !dir.exists() {
        return Ok(copybooks);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if matches!(ext_lower.as_str(), "cpy" | "copy") {
                    copybooks.push(path);
                }
            }
        }
    }

    Ok(copybooks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_copy_references() {
        let source = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TEST-PROG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       COPY CUSTOMER-RECORD.
       PROCEDURE DIVISION.
           DISPLAY "HELLO".
           STOP RUN.
"#;
        let refs = CopybookResolver::find_copy_references(source);
        assert_eq!(refs, vec!["CUSTOMER-RECORD"]);
    }

    #[test]
    fn test_resolve_no_copy() {
        let source = "       DISPLAY \"HELLO\".\n";
        let mut resolver = CopybookResolver::new(vec![]);
        let result = resolver.resolve(source).unwrap();
        assert_eq!(result, "       DISPLAY \"HELLO\".\n");
    }
}
