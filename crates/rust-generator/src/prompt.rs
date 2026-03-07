use cobol_parser::CobolProgram;

/// Build the system prompt for COBOL-to-Rust conversion.
pub fn build_system_prompt() -> String {
    r#"You are an expert COBOL-to-Rust converter. You convert COBOL programs to idiomatic, correct Rust code.

## Critical Rules

1. **Financial precision**: NEVER use f32 or f64 for numeric values that have decimal places in COBOL PIC clauses. Always use `rust_decimal::Decimal` for PIC 9(n)V9(m) patterns.
2. **Output must be identical**: The Rust program must produce EXACTLY the same output as the COBOL program, character for character.
3. **Complete programs**: Generate complete, compilable Rust programs with `fn main()` and all necessary `use` statements.
4. **Error handling**: Use `anyhow::Result` for error handling where appropriate.

## PIC Clause Mapping

- `PIC 9(n)` (no decimal) → Use appropriate integer type (u8, u16, u32, u64) based on digit count
- `PIC 9(n)V9(m)` → `rust_decimal::Decimal` with correct scale
- `PIC S9(n)V9(m)` → `rust_decimal::Decimal` (signed)
- `PIC X(n)` → `String`
- `PIC A(n)` → `String`
- `PIC 9` → `u8`

## COBOL Statement Mapping

- `DISPLAY` → `println!()` or `print!()`
- `MOVE x TO y` → `y = x` (with appropriate type conversion)
- `ADD x TO y` → `y += x` or `y = y + x`
- `SUBTRACT x FROM y` → `y -= x`
- `MULTIPLY x BY y` → `y *= x`
- `DIVIDE x INTO y` → `y /= x`
- `COMPUTE x = expr` → `x = expr` (translate arithmetic operators)
- `PERFORM paragraph` → function call
- `PERFORM VARYING` → `for` loop
- `PERFORM UNTIL` → `while` or `loop` with condition
- `STOP RUN` → `return` or process exit
- `IF...THEN...ELSE...END-IF` → `if...else`

## Output Format

Return ONLY the Rust source code, wrapped in ```rust ... ``` markers. Do not include explanations outside the code block."#.to_string()
}

/// Build the user prompt for a specific COBOL program conversion.
pub fn build_conversion_prompt(program: &CobolProgram, source: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("Convert the following COBOL program to Rust.\n\n");
    prompt.push_str(&format!("Program ID: {}\n", program.program_id));

    if let Some(ref data) = program.data {
        if !data.working_storage.is_empty() {
            prompt.push_str(&format!(
                "Data items: {} variables in WORKING-STORAGE\n",
                data.working_storage.len()
            ));
        }
    }

    if let Some(ref proc) = program.procedure {
        prompt.push_str(&format!("Paragraphs: {}\n", proc.paragraphs.len()));
    }

    prompt.push_str("\n## COBOL Source Code\n\n```cobol\n");
    prompt.push_str(source);
    prompt.push_str("\n```\n\n");

    prompt.push_str("Generate the equivalent Rust program. ");
    prompt.push_str(
        "The output must be character-for-character identical to the COBOL program's output. ",
    );
    prompt.push_str("Use rust_decimal::Decimal for any PIC clause with decimal positions (V).\n");

    prompt
}

/// Build a fix prompt when the generated code has compilation errors.
pub fn build_fix_prompt(rust_code: &str, errors: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("The following Rust code has compilation errors. Fix them.\n\n");
    prompt.push_str("## Current Rust Code\n\n```rust\n");
    prompt.push_str(rust_code);
    prompt.push_str("\n```\n\n");
    prompt.push_str("## Compilation Errors\n\n```\n");
    prompt.push_str(errors);
    prompt.push_str("\n```\n\n");
    prompt.push_str(
        "Return the COMPLETE fixed Rust source code wrapped in ```rust ... ``` markers.\n",
    );

    prompt
}

/// Build a fix prompt when the output doesn't match.
pub fn build_output_fix_prompt(rust_code: &str, expected: &str, actual: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("The Rust code compiles but produces incorrect output. Fix it.\n\n");
    prompt.push_str("## Current Rust Code\n\n```rust\n");
    prompt.push_str(rust_code);
    prompt.push_str("\n```\n\n");
    prompt.push_str("## Expected Output\n\n```\n");
    prompt.push_str(expected);
    prompt.push_str("\n```\n\n");
    prompt.push_str("## Actual Output\n\n```\n");
    prompt.push_str(actual);
    prompt.push_str("\n```\n\n");
    prompt.push_str(
        "Return the COMPLETE fixed Rust source code wrapped in ```rust ... ``` markers.\n",
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_contains_key_rules() {
        let prompt = build_system_prompt();
        assert!(prompt.contains("rust_decimal"));
        assert!(prompt.contains("NEVER use f32 or f64"));
        assert!(prompt.contains("PIC"));
    }

    #[test]
    fn test_fix_prompt() {
        let prompt = build_fix_prompt("fn main() { x }", "error: cannot find value `x`");
        assert!(prompt.contains("compilation errors"));
        assert!(prompt.contains("cannot find value"));
    }
}
