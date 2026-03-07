// Converted from: 05_copybook/input.cob + CUSTOMER.cpy
// PROGRAM-ID: COPYBOOK-TEST
//
// COPY CUSTOMER expands to:
//   01 CUSTOMER-RECORD.
//     05 CUST-ID   PIC 9(5).
//     05 CUST-NAME PIC X(30).
//
// The COPY statement is resolved by the parser; struct is generated from copybook.

struct CustomerRecord {
    cust_id: u32,       // PIC 9(5)
    cust_name: String,  // PIC X(30) — right-padded with spaces to 30 chars
}

impl CustomerRecord {
    fn new() -> Self {
        CustomerRecord {
            cust_id: 0,
            cust_name: " ".repeat(30),
        }
    }

    /// Display PIC 9(5): zero-padded integer
    fn display_id(&self) -> String {
        format!("{:05}", self.cust_id)
    }

    /// Display PIC X(30): right-padded to 30, then rtrimmed for display
    fn display_name(&self) -> String {
        let padded = format!("{:<30}", self.cust_name);
        // COBOL DISPLAY of PIC X(N) includes trailing spaces; we trim for readability
        padded.trim_end().to_string()
    }
}

fn main() {
    let mut customer = CustomerRecord::new();

    // MOVE "JOHN DOE" TO CUST-NAME
    customer.cust_name = "JOHN DOE".to_string();
    // MOVE 12345 TO CUST-ID
    customer.cust_id = 12345;

    // DISPLAY "NAME: " CUST-NAME
    println!("NAME: {}", customer.display_name());
    // DISPLAY "ID:   " CUST-ID
    println!("ID:   {}", customer.display_id());
}
