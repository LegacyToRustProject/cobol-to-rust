use std::path::Path;

use anyhow::{Context, Result};
use tracing::{info, warn};

use rust_generator::llm::{LlmProvider, LlmRequest};
use rust_generator::prompt::{build_fix_prompt, build_output_fix_prompt, build_system_prompt};

use crate::comparator::{compare_outputs, format_diff};
use crate::compiler::{cargo_check, create_temp_project};

/// Maximum number of fix attempts before giving up.
const MAX_FIX_ATTEMPTS: u32 = 5;

/// Result of the verification and fix loop.
#[derive(Debug)]
pub struct VerifyResult {
    pub success: bool,
    pub rust_code: String,
    pub attempts: u32,
    pub final_error: Option<String>,
}

/// Run the verify-and-fix loop:
/// 1. cargo check the generated code
/// 2. If it fails, ask the LLM to fix it
/// 3. Repeat until success or max attempts
pub async fn verify_and_fix(
    llm: &dyn LlmProvider,
    initial_code: &str,
    expected_output: Option<&str>,
    work_dir: &Path,
) -> Result<VerifyResult> {
    let mut current_code = initial_code.to_string();
    let mut attempts = 0u32;

    loop {
        attempts += 1;
        info!(attempt = attempts, "Verification attempt");

        // Step 1: Create temp project and check compilation
        let project_dir = work_dir.join(format!("attempt_{attempts}"));
        create_temp_project(&current_code, &project_dir)
            .context("Failed to create temp project")?;

        let check_result = cargo_check(&project_dir)?;

        if !check_result.success {
            warn!(attempt = attempts, "Compilation failed");

            if attempts >= MAX_FIX_ATTEMPTS {
                return Ok(VerifyResult {
                    success: false,
                    rust_code: current_code,
                    attempts,
                    final_error: Some(format!("Compilation failed: {}", check_result.stderr)),
                });
            }

            // Ask LLM to fix
            current_code = ask_llm_to_fix(llm, &current_code, &check_result.stderr).await?;
            continue;
        }

        info!(attempt = attempts, "Compilation successful");

        // Step 2: If we have expected output, run and compare
        if let Some(expected) = expected_output {
            let run_result = run_cargo_project(&project_dir)?;

            if !run_result.success {
                warn!("Runtime error: {}", run_result.stderr);

                if attempts >= MAX_FIX_ATTEMPTS {
                    return Ok(VerifyResult {
                        success: false,
                        rust_code: current_code,
                        attempts,
                        final_error: Some(format!("Runtime error: {}", run_result.stderr)),
                    });
                }

                current_code = ask_llm_to_fix(llm, &current_code, &run_result.stderr).await?;
                continue;
            }

            let comparison = compare_outputs(expected, &run_result.stdout);
            if !comparison.matches {
                let diff = format_diff(&comparison);
                warn!("Output mismatch:\n{}", diff);

                if attempts >= MAX_FIX_ATTEMPTS {
                    return Ok(VerifyResult {
                        success: false,
                        rust_code: current_code,
                        attempts,
                        final_error: Some(format!("Output mismatch: {diff}")),
                    });
                }

                current_code =
                    ask_llm_to_fix_output(llm, &current_code, expected, &run_result.stdout).await?;
                continue;
            }

            info!("Output matches expected!");
        }

        return Ok(VerifyResult {
            success: true,
            rust_code: current_code,
            attempts,
            final_error: None,
        });
    }
}

struct RunResult {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_cargo_project(project_dir: &Path) -> Result<RunResult> {
    let output = std::process::Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .current_dir(project_dir)
        .output()
        .context("Failed to run cargo run")?;

    Ok(RunResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

async fn ask_llm_to_fix(llm: &dyn LlmProvider, code: &str, errors: &str) -> Result<String> {
    let request = LlmRequest {
        system_prompt: build_system_prompt(),
        user_prompt: build_fix_prompt(code, errors),
        max_tokens: 8192,
        temperature: 0.0,
    };

    let response = llm.generate(&request).await?;
    rust_generator::generator::extract_rust_code(&response.content)
}

async fn ask_llm_to_fix_output(
    llm: &dyn LlmProvider,
    code: &str,
    expected: &str,
    actual: &str,
) -> Result<String> {
    let request = LlmRequest {
        system_prompt: build_system_prompt(),
        user_prompt: build_output_fix_prompt(code, expected, actual),
        max_tokens: 8192,
        temperature: 0.0,
    };

    let response = llm.generate(&request).await?;
    rust_generator::generator::extract_rust_code(&response.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_fix_attempts_constant() {
        assert!(MAX_FIX_ATTEMPTS >= 3);
        assert!(MAX_FIX_ATTEMPTS <= 10);
    }
}
