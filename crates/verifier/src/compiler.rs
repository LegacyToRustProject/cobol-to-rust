use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use tracing::info;

/// Result of a compilation check.
#[derive(Debug)]
pub struct CompileResult {
    pub success: bool,
    pub stderr: String,
    pub stdout: String,
}

/// Run `cargo check` on a generated Rust project.
pub fn cargo_check(project_dir: &Path) -> Result<CompileResult> {
    info!(dir = %project_dir.display(), "Running cargo check");

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(project_dir)
        .output()
        .context("Failed to execute cargo check")?;

    Ok(CompileResult {
        success: output.status.success(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    })
}

/// Compile and run a Rust source file, returning its output.
pub fn compile_and_run(source_path: &Path) -> Result<CompileResult> {
    let output_binary = source_path.with_extension("");

    // Compile
    let compile = Command::new("rustc")
        .arg(source_path)
        .arg("-o")
        .arg(&output_binary)
        .output()
        .context("Failed to execute rustc")?;

    if !compile.status.success() {
        return Ok(CompileResult {
            success: false,
            stderr: String::from_utf8_lossy(&compile.stderr).to_string(),
            stdout: String::from_utf8_lossy(&compile.stdout).to_string(),
        });
    }

    // Run
    let run = Command::new(&output_binary)
        .output()
        .context("Failed to run compiled binary")?;

    // Clean up binary
    let _ = std::fs::remove_file(&output_binary);

    Ok(CompileResult {
        success: run.status.success(),
        stderr: String::from_utf8_lossy(&run.stderr).to_string(),
        stdout: String::from_utf8_lossy(&run.stdout).to_string(),
    })
}

/// Compile and run a COBOL source file using GnuCOBOL, returning its output.
pub fn compile_and_run_cobol(source_path: &Path) -> Result<CompileResult> {
    let cobc = std::env::var("COBOL_COMPILER").unwrap_or_else(|_| "cobc".to_string());
    let output_binary = source_path.with_extension("");

    // Compile with GnuCOBOL
    let compile = Command::new(&cobc)
        .arg("-x") // Create executable
        .arg("-o")
        .arg(&output_binary)
        .arg(source_path)
        .output()
        .context("Failed to execute GnuCOBOL compiler (cobc). Is gnucobol installed?")?;

    if !compile.status.success() {
        return Ok(CompileResult {
            success: false,
            stderr: String::from_utf8_lossy(&compile.stderr).to_string(),
            stdout: String::from_utf8_lossy(&compile.stdout).to_string(),
        });
    }

    // Run
    let run = Command::new(&output_binary)
        .output()
        .context("Failed to run compiled COBOL binary")?;

    // Clean up binary
    let _ = std::fs::remove_file(&output_binary);

    Ok(CompileResult {
        success: run.status.success(),
        stderr: String::from_utf8_lossy(&run.stderr).to_string(),
        stdout: String::from_utf8_lossy(&run.stdout).to_string(),
    })
}

/// Create a temporary Cargo project with the given Rust source code.
pub fn create_temp_project(code: &str, project_dir: &Path) -> Result<()> {
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Check if code uses rust_decimal
    let needs_decimal = code.contains("rust_decimal") || code.contains("Decimal");
    let needs_anyhow = code.contains("anyhow");

    let mut deps = String::new();
    if needs_decimal {
        deps.push_str("rust_decimal = \"1\"\n");
    }
    if needs_anyhow {
        deps.push_str("anyhow = \"1\"\n");
    }

    let cargo_toml = format!(
        r#"[package]
name = "cobol-converted"
version = "0.1.0"
edition = "2021"

[dependencies]
{deps}
"#
    );

    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(src_dir.join("main.rs"), code)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_temp_project() {
        let dir = TempDir::new().unwrap();
        let code = r#"fn main() { println!("hello"); }"#;
        create_temp_project(code, dir.path()).unwrap();

        assert!(dir.path().join("Cargo.toml").exists());
        assert!(dir.path().join("src/main.rs").exists());
    }

    #[test]
    fn test_create_temp_project_with_decimal() {
        let dir = TempDir::new().unwrap();
        let code = r#"use rust_decimal::Decimal; fn main() {}"#;
        create_temp_project(code, dir.path()).unwrap();

        let toml = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert!(toml.contains("rust_decimal"));
    }
}
