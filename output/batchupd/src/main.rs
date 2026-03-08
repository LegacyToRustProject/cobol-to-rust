// Converted from: BATCHUPD.cbl — batch financial transaction processor
// PROGRAM-ID: BATCHUPD
//
// IMPORTANT: Record layout discrepancy noted in source COBOL:
//   COBOL spec: TR-ACCT PIC 9(10)=10 + TR-TYPE PIC X=1 + TR-AMOUNT PIC 9(9)V99=11 = 22 chars
//   INPUT-RECORD PIC X(21) = 21 chars (spec inconsistency: 22 > 21)
//   Test data "12345678901D000000099" = 21 chars; actual layout used:
//     TR-ACCT [0..11]  = 11 chars (overflow from PIC 9(10) in test data)
//     TR-TYPE [11]     = 1 char ('D'=deposit, 'W'=withdrawal)
//     TR-AMOUNT [12..21] = 9 chars (last 2 digits are cents, i.e., PIC 9(7)V99)
//
// This discrepancy is documented in oss-conversion-report.md.
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::io::{self, BufRead, Write};

/// Parse a 9-char COBOL amount field where last 2 digits are cents.
/// e.g., "000000099" -> 0.99, "000000050" -> 0.50
fn parse_amount(raw: &str) -> Decimal {
    let raw = raw.trim();
    if raw.len() < 2 {
        return dec!(0);
    }
    let (int_part, dec_part) = raw.split_at(raw.len() - 2);
    let int_val: u64 = int_part.parse().unwrap_or(0);
    let dec_val: u64 = dec_part.parse().unwrap_or(0);
    Decimal::from(int_val) + Decimal::from(dec_val) / Decimal::from(100u64)
}

/// Format PIC S9(11)V99 for DISPLAY.
fn format_balance(bal: Decimal) -> String {
    let abs_bal = bal.abs();
    let factor = Decimal::from(100u64);
    let truncated = (abs_bal * factor).trunc() / factor;
    let s = format!("{:.2}", truncated);
    let (int_str, dec_str) = s.split_once('.').unwrap_or((&s, "00"));
    let formatted = format!("{:0>11}.{}", int_str, dec_str);
    if bal < dec!(0) {
        format!("-{}", formatted)
    } else {
        formatted
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut ws_balance = dec!(0);

    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.len() < 13 { continue; }

        let tr_acct = &line[0..11];
        let tr_type = line.chars().nth(11).unwrap_or(' ');
        let tr_amount_raw = if line.len() >= 21 { &line[12..21] } else { &line[12..] };
        let tr_amount = parse_amount(tr_amount_raw);

        if tr_type == 'D' {
            ws_balance += tr_amount;
        } else if tr_type == 'W' {
            ws_balance -= tr_amount;
        }

        let acct_display = tr_acct.trim_start_matches('0');
        writeln!(out, "ACCT:{} BAL:{}", acct_display, format_balance(ws_balance)).unwrap();
    }
}
