// Converted from: CBL0002.cbl — EVALUATE (switch/case)
// PROGRAM-ID: CBL0002
fn main() {
    let ws_grade: u32 = 85;   // PIC 9(3)
    let ws_letter: char;       // PIC X

    // EVALUATE TRUE
    ws_letter = if ws_grade >= 90 {
        'A'
    } else if ws_grade >= 80 {
        'B'
    } else if ws_grade >= 70 {
        'C'
    } else if ws_grade >= 60 {
        'D'
    } else {
        'F'
    };

    // DISPLAY "GRADE: " WS-GRADE (PIC 9(3))
    println!("GRADE: {:03}", ws_grade);
    // DISPLAY "LETTER: " WS-LETTER (PIC X)
    println!("LETTER: {}", ws_letter);
}
