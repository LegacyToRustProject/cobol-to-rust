// Converted from: CBL0003.cbl — PERFORM VARYING with accumulator
// PROGRAM-ID: CBL0003
fn accumulate(counter: u8, total: &mut u32) {
    // ADD WS-COUNTER TO WS-TOTAL
    *total += counter as u32;
}

fn main() {
    let mut ws_total: u32 = 0; // PIC 9(4)

    // PERFORM ACCUMULATE VARYING WS-COUNTER FROM 1 BY 1 UNTIL WS-COUNTER > 10
    let mut ws_counter: u8 = 1;
    while ws_counter <= 10 {
        accumulate(ws_counter, &mut ws_total);
        ws_counter += 1;
    }

    // DISPLAY "SUM 1-10: " WS-TOTAL (PIC 9(4))
    println!("SUM 1-10: {:04}", ws_total);
}
