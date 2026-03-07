// Converted from: 03_perform_loop/input.cob
// PROGRAM-ID: PERFORM-LOOP
//
// PERFORM para VARYING counter FROM 1 BY 1 UNTIL counter > limit
// PIC 9(2): 2 digits with leading zeros

fn display_line(counter: u8) {
    // DISPLAY "COUNT: " WS-COUNTER (PIC 9(2))
    println!("COUNT: {:02}", counter);
}

fn main() {
    // 01 WS-COUNTER PIC 9(2) VALUE 0
    // 01 WS-LIMIT   PIC 9(2) VALUE 5
    let ws_limit: u8 = 5;

    // PERFORM DISPLAY-LINE VARYING WS-COUNTER FROM 1 BY 1 UNTIL WS-COUNTER > WS-LIMIT
    let mut ws_counter: u8 = 1;
    while ws_counter <= ws_limit {
        display_line(ws_counter);
        ws_counter += 1;
    }
}
