// Converted from: CBL0004.cbl — sequential file I/O with record count
// PROGRAM-ID: CBL0004
//
// SELECT NAMES-FILE ASSIGN TO "names.dat"
// FD NAMES-FILE, PIC X(30): read each record
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    let mut ws_count: u32 = 0; // PIC 9(3)

    let file = match File::open("names.dat") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening names.dat: {}", e);
            std::process::exit(1);
        }
    };
    let reader = BufReader::new(file);

    // PERFORM READ-NAMES UNTIL WS-EOF = 1
    for line in reader.lines() {
        match line {
            Ok(record) => {
                ws_count += 1; // ADD 1 TO WS-COUNT
                // DISPLAY "NAME: " NAME-RECORD (PIC X(30), rtrimmed)
                println!("NAME: {:<30}", record.trim_end());
            }
            Err(_) => break,
        }
    }

    // DISPLAY "TOTAL NAMES: " WS-COUNT (PIC 9(3))
    println!("TOTAL NAMES: {:03}", ws_count);
}
