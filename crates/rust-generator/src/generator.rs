use std::path::Path;

use anyhow::{Context, Result};
use regex::Regex;
use tracing::info;

use cobol_parser::{parse_cobol_source, CobolProgram, CopybookResolver};

use crate::llm::{LlmProvider, LlmRequest};
use crate::prompt::{build_conversion_prompt, build_system_prompt};

/// Result of a code generation attempt.
pub struct GenerationResult {
    pub rust_code: String,
    pub program: CobolProgram,
    pub tokens_used: Option<u32>,
}

/// Orchestrates the COBOL-to-Rust conversion process.
pub struct Generator {
    llm: Box<dyn LlmProvider>,
    copybook_paths: Vec<std::path::PathBuf>,
}

impl Generator {
    pub fn new(llm: Box<dyn LlmProvider>, copybook_paths: Vec<std::path::PathBuf>) -> Self {
        Self {
            llm,
            copybook_paths,
        }
    }

    /// Convert a single COBOL source file to Rust.
    pub async fn convert(&self, source_path: &Path) -> Result<GenerationResult> {
        let raw_source = std::fs::read_to_string(source_path)
            .with_context(|| format!("Failed to read {}", source_path.display()))?;

        // Resolve COPY statements
        let mut resolver = CopybookResolver::new(self.copybook_paths.clone());
        let resolved_source = resolver
            .resolve(&raw_source)
            .context("Failed to resolve COPY statements")?;

        // Parse the COBOL source
        let program =
            parse_cobol_source(&resolved_source).context("Failed to parse COBOL source")?;

        info!(
            program_id = %program.program_id,
            "Parsed COBOL program, sending to LLM for conversion"
        );

        // Build prompts
        let system_prompt = build_system_prompt();
        let user_prompt = build_conversion_prompt(&program, &resolved_source);

        // Call LLM
        let request = LlmRequest {
            system_prompt,
            user_prompt,
            max_tokens: 8192,
            temperature: 0.0,
        };

        let response = self
            .llm
            .generate(&request)
            .await
            .context("LLM generation failed")?;

        // Extract Rust code from response
        let rust_code = extract_rust_code(&response.content)
            .context("Failed to extract Rust code from LLM response")?;

        info!(
            tokens = response.tokens_used,
            code_len = rust_code.len(),
            "Generated Rust code"
        );

        Ok(GenerationResult {
            rust_code,
            program,
            tokens_used: response.tokens_used,
        })
    }

    /// Convert COBOL source from a string (for testing).
    pub async fn convert_source(&self, source: &str) -> Result<GenerationResult> {
        let mut resolver = CopybookResolver::new(self.copybook_paths.clone());
        let resolved_source = resolver.resolve(source)?;
        let program = parse_cobol_source(&resolved_source)?;

        let system_prompt = build_system_prompt();
        let user_prompt = build_conversion_prompt(&program, &resolved_source);

        let request = LlmRequest {
            system_prompt,
            user_prompt,
            max_tokens: 8192,
            temperature: 0.0,
        };

        let response = self.llm.generate(&request).await?;
        let rust_code = extract_rust_code(&response.content)?;

        Ok(GenerationResult {
            rust_code,
            program,
            tokens_used: response.tokens_used,
        })
    }
}

/// Extract Rust code from an LLM response that may contain markdown code blocks.
pub fn extract_rust_code(response: &str) -> Result<String> {
    // Try to find ```rust ... ``` block
    let re = Regex::new(r"```rust\s*\n([\s\S]*?)```").unwrap();
    if let Some(cap) = re.captures(response) {
        return Ok(cap[1].trim().to_string());
    }

    // Try generic ``` block
    let re = Regex::new(r"```\s*\n([\s\S]*?)```").unwrap();
    if let Some(cap) = re.captures(response) {
        return Ok(cap[1].trim().to_string());
    }

    // If no code blocks, check if the whole response looks like Rust code
    if response.contains("fn main()") || response.contains("fn main (") {
        return Ok(response.trim().to_string());
    }

    anyhow::bail!("No Rust code found in LLM response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_code_with_markers() {
        let response = r#"Here is the converted code:

```rust
fn main() {
    println!("HELLO WORLD");
}
```

This is the equivalent Rust program."#;
        let code = extract_rust_code(response).unwrap();
        assert!(code.contains("fn main()"));
        assert!(code.contains("println!"));
        assert!(!code.contains("```"));
    }

    #[test]
    fn test_extract_rust_code_generic_block() {
        let response = "```\nfn main() {\n    println!(\"test\");\n}\n```";
        let code = extract_rust_code(response).unwrap();
        assert!(code.contains("fn main()"));
    }

    #[test]
    fn test_extract_rust_code_no_markers() {
        let response = "fn main() {\n    println!(\"test\");\n}";
        let code = extract_rust_code(response).unwrap();
        assert!(code.contains("fn main()"));
    }

    #[test]
    fn test_extract_rust_code_fails_on_garbage() {
        let result = extract_rust_code("No code here, just text.");
        assert!(result.is_err());
    }
}
