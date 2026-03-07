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

## Version Compatibility

COBOL is unique: **the oldest version is the most common.** Most production systems run COBOL-85 or earlier dialects.

| COBOL Standard | Priority | Notes |
|----------------|----------|-------|
| COBOL-85 | **First** | De facto standard. 90%+ of production COBOL. |
| VS COBOL II (IBM) | **First** | IBM mainframe dialect. Banking standard. |
| Enterprise COBOL (IBM) | Second | Modern IBM dialect with some OOP. |
| COBOL 2002 | Third | OOP extensions. Rarely used in legacy. |
| COBOL 2014/2023 | Fourth | Modern features. Almost no adoption. |
| Micro Focus COBOL | Second | Common on distributed (non-mainframe) systems. |

Unlike other languages, older = simpler = easier to convert. COBOL-85 has no objects, no exceptions, and predictable control flow. This is the sweet spot.

Auto-detection: `cobol-to-rust analyze` detects the COBOL dialect and compiler-specific extensions.

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
