// Converted from: CBL0001.cbl — variables and arithmetic
// PROGRAM-ID: CBL0001
//
// PIC 9(4) → 4-digit zero-padded unsigned
// PIC 9(5) → 5-digit zero-padded
// PIC S9(5) → signed 5-digit (sign prefix when negative)
// PIC 9(9) → 9-digit zero-padded

fn main() {
    let ws_num1: i64 = 1234; // PIC 9(4)
    let ws_num2: i64 = 5678; // PIC 9(4)

    // COMPUTE WS-SUM = WS-NUM1 + WS-NUM2  (PIC 9(5))
    let ws_sum: i64 = ws_num1 + ws_num2;
    // COMPUTE WS-DIFF = WS-NUM1 - WS-NUM2  (PIC S9(5))
    let ws_diff: i64 = ws_num1 - ws_num2;
    // COMPUTE WS-PRODUCT = WS-NUM1 * WS-NUM2  (PIC 9(9))
    let ws_product: i64 = ws_num1 * ws_num2;

    println!("NUM1:    {:04}", ws_num1);
    println!("NUM2:    {:04}", ws_num2);
    println!("SUM:     {:05}", ws_sum);
    // PIC S9(5): signed, zero-padded (e.g., -04444)
    if ws_diff < 0 {
        println!("DIFF:    -{:05}", ws_diff.unsigned_abs());
    } else {
        println!("DIFF:    {:05}", ws_diff);
    }
    println!("PRODUCT: {:09}", ws_product);
}
