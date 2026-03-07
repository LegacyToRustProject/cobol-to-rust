use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser)]
#[command(name = "cobol-to-rust")]
#[command(about = "Convert COBOL programs to Rust using AI")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a COBOL project and produce a structure report
    Analyze {
        /// Path to a COBOL source file or directory
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Convert a COBOL program to Rust
    Convert {
        /// Path to a COBOL source file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output directory for the generated Rust code
        #[arg(short, long, default_value = "output")]
        output: PathBuf,

        /// Directories to search for COPYBOOK files
        #[arg(short, long)]
        copybook_path: Vec<PathBuf>,

        /// Expected output file for verification
        #[arg(short, long)]
        expected_output: Option<PathBuf>,

        /// Skip the AI fix loop (just generate once)
        #[arg(long)]
        no_fix_loop: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path } => cmd_analyze(&path),
        Commands::Convert {
            input,
            output,
            copybook_path,
            expected_output,
            no_fix_loop,
        } => cmd_convert(&input, &output, copybook_path, expected_output, no_fix_loop).await,
    }
}

fn cmd_analyze(path: &Path) -> Result<()> {
    info!(path = %path.display(), "Analyzing COBOL project");

    let mut programs = Vec::new();
    let mut copybooks = Vec::new();
    let mut total_lines = 0usize;

    let files = collect_cobol_files(path)?;

    if files.is_empty() {
        anyhow::bail!("No COBOL files found at {}", path.display());
    }

    for file in &files {
        let source = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;
        total_lines += source.lines().count();

        match cobol_parser::analyze_file(file, &source) {
            Ok(summary) => programs.push(summary),
            Err(e) => {
                eprintln!("Warning: Failed to analyze {}: {e}", file.display());
            }
        }

        // Check for COPY references
        let refs = cobol_parser::CopybookResolver::find_copy_references(&source);
        for r in refs {
            copybooks.push(cobol_parser::CopybookInfo {
                name: r.clone(),
                file_path: String::new(),
                referenced_by: vec![file.to_string_lossy().to_string()],
            });
        }
    }

    let complexity = if total_lines > 10000 {
        cobol_parser::ComplexityLevel::Complex
    } else if total_lines > 1000 {
        cobol_parser::ComplexityLevel::Moderate
    } else {
        cobol_parser::ComplexityLevel::Simple
    };

    let report = cobol_parser::AnalysisReport {
        programs,
        copybooks,
        total_lines,
        complexity,
    };

    let json = serde_json::to_string_pretty(&report)?;
    println!("{json}");

    Ok(())
}

async fn cmd_convert(
    input: &Path,
    output: &Path,
    copybook_paths: Vec<PathBuf>,
    expected_output: Option<PathBuf>,
    no_fix_loop: bool,
) -> Result<()> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    info!(input = %input.display(), "Converting COBOL to Rust");

    let llm = rust_generator::ClaudeProvider::new(api_key);
    let generator = rust_generator::Generator::new(Box::new(llm), copybook_paths.clone());

    let result = generator
        .convert(input)
        .await
        .context("Conversion failed")?;

    info!(
        program_id = %result.program.program_id,
        tokens = result.tokens_used,
        "Initial conversion complete"
    );

    let final_code = if no_fix_loop {
        result.rust_code
    } else {
        let expected = if let Some(ref path) = expected_output {
            Some(std::fs::read_to_string(path).context("Failed to read expected output file")?)
        } else {
            None
        };

        let work_dir = tempfile::tempdir()?;
        let llm = rust_generator::ClaudeProvider::new(std::env::var("ANTHROPIC_API_KEY").unwrap());

        let verify_result = verifier::verify_and_fix(
            &llm,
            &result.rust_code,
            expected.as_deref(),
            work_dir.path(),
        )
        .await?;

        if verify_result.success {
            info!(attempts = verify_result.attempts, "Verification succeeded!");
        } else {
            eprintln!(
                "Warning: Verification failed after {} attempts: {}",
                verify_result.attempts,
                verify_result.final_error.as_deref().unwrap_or("unknown")
            );
        }

        verify_result.rust_code
    };

    // Write output
    std::fs::create_dir_all(output)?;
    let output_file = output.join("main.rs");
    std::fs::write(&output_file, &final_code)?;

    info!(output = %output_file.display(), "Rust code written");
    println!("Conversion complete: {}", output_file.display());

    Ok(())
}

fn collect_cobol_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_lower.as_str(), "cob" | "cbl" | "cobol") {
                        files.push(p);
                    }
                }
            }
        }
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    Ok(files)
}
