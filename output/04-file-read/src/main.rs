// Converted from: 04_file_read/input.cob
// PROGRAM-ID: FILE-READ
//
// FD INPUT-FILE, PIC X(80): reads sequential file, displays each record
// File is hardcoded to "test_data.dat" (relative path)
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    // OPEN INPUT INPUT-FILE (SELECT assigned to "test_data.dat")
    let file = match File::open("test_data.dat") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening test_data.dat: {}", e);
            std::process::exit(1);
        }
    };
    let reader = BufReader::new(file);

    // PERFORM READ-LOOP UNTIL WS-EOF = 1
    for line in reader.lines() {
        match line {
            Ok(record) => {
                // DISPLAY INPUT-RECORD — PIC X(80), pad/truncate to 80 chars then rtrim
                // COBOL DISPLAY of X(80) shows all 80 chars; we display as-is (rtrimmed for readability)
                println!("{}", record);
            }
            Err(_) => break,
        }
    }
    // CLOSE INPUT-FILE / STOP RUN
}
