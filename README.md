# cobol-to-rust

**AI-powered COBOL → Rust conversion agent.**

## Why COBOL

- 95% of ATM transactions run on COBOL
- Estimated 240 billion lines of COBOL in production
- Average COBOL developer age: 60+. They're retiring. No one is replacing them.
- The source code IS the spec. Documentation is decades out of date or lost.

## How It Works

```
COBOL program (source + running instance)
    ↓ 1. Parse & analyze (COPY books, file layouts, PERFORM graphs)
    ↓ 2. AI converts each paragraph/section to Rust
    ↓ 3. cargo check (must compile)
    ↓ 4. Run both COBOL & Rust with same inputs, compare outputs
    ↓ 5. Diff? → AI fixes → goto 3
    ↓ 6. Repeat until all outputs match
Verified Rust binary
```

## Key Challenges

| COBOL Feature | Conversion Strategy |
|---|---|
| Fixed-point decimal (PIC 9) | `rust_decimal` or custom types |
| COPY books (shared layouts) | Rust structs with `#[repr(C)]` |
| File I/O (ISAM, VSAM) | Rust file/DB abstraction |
| PERFORM VARYING | Idiomatic Rust loops |
| REDEFINES | Rust enums or unions |
| Batch JCL integration | Shell scripts / workflow engine |

## Target Industries

- Banking & Finance
- Insurance
- Government / Public sector
- Healthcare

## Status

**Concept.** Architecture design in progress.

## Part of [LegacyToRust Project](https://github.com/LegacyToRustProject)

## License

MIT
