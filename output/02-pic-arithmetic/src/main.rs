// Converted from: 02_pic_arithmetic/input.cob
// PROGRAM-ID: PIC-ARITHMETIC
//
// PIC 9(N)VDD: N integer digits (leading zeros) + implicit decimal + D decimal places
// COMPUTE without ROUNDED: truncates (does not round)
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Format a Decimal as COBOL PIC 9(int_digits)V9(dec_digits).
/// Truncates to dec_digits decimal places (COBOL COMPUTE without ROUNDED).
fn pic_format(val: Decimal, int_digits: usize, dec_digits: usize) -> String {
    // Truncate to dec_digits decimal places
    let factor = Decimal::from(10u64.pow(dec_digits as u32));
    let truncated = (val * factor).trunc() / factor;

    // Format with exact decimal places
    let s = format!("{:.prec$}", truncated, prec = dec_digits);
    let (int_str, dec_str) = s.split_once('.').unwrap_or((&s, ""));

    // Zero-pad integer part to int_digits
    format!("{:0>width$}.{}", int_str, dec_str, width = int_digits)
}

fn main() {
    // 01 WS-PRICE  PIC 9(5)V99 VALUE 12345.67
    let ws_price = dec!(12345.67);
    // 01 WS-TAX-RATE PIC 9V99 VALUE 0.08
    let ws_tax_rate = dec!(0.08);

    // COMPUTE WS-TAX = WS-PRICE * WS-TAX-RATE  (PIC 9(5)V99 → truncate to 2dp)
    let factor = Decimal::from(100u64);
    let ws_tax = (ws_price * ws_tax_rate * factor).trunc() / factor;

    // COMPUTE WS-TOTAL = WS-PRICE + WS-TAX  (PIC 9(6)V99 → truncate to 2dp)
    let ws_total = ((ws_price + ws_tax) * factor).trunc() / factor;

    println!("PRICE: {}", pic_format(ws_price, 5, 2));
    println!("TAX:   {}", pic_format(ws_tax, 5, 2));
    println!("TOTAL: {}", pic_format(ws_total, 6, 2));
}
