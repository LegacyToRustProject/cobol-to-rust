use cobol_parser::types::{PicClause, PicType};

/// Determine the Rust type for a COBOL PIC clause.
pub fn rust_type_for_pic(pic: &PicClause) -> String {
    match pic.pic_type {
        PicType::Alphanumeric | PicType::Alphabetic => "String".to_string(),
        PicType::Numeric => {
            if pic.decimal_digits > 0 {
                // Must use Decimal for financial precision
                "Decimal".to_string()
            } else if pic.signed {
                match pic.integer_digits {
                    0..=2 => "i8".to_string(),
                    3..=4 => "i16".to_string(),
                    5..=9 => "i32".to_string(),
                    _ => "i64".to_string(),
                }
            } else {
                match pic.integer_digits {
                    0..=2 => "u8".to_string(),
                    3..=4 => "u16".to_string(),
                    5..=9 => "u32".to_string(),
                    _ => "u64".to_string(),
                }
            }
        }
    }
}

/// Generate the Rust default value expression for a PIC clause.
pub fn rust_default_for_pic(pic: &PicClause, value: Option<&str>) -> String {
    match pic.pic_type {
        PicType::Alphanumeric | PicType::Alphabetic => {
            if let Some(val) = value {
                let cleaned = val.trim_matches('"').trim_matches('\'');
                format!("\"{}\".to_string()", cleaned)
            } else {
                format!("String::with_capacity({})", pic.total_size)
            }
        }
        PicType::Numeric => {
            if pic.decimal_digits > 0 {
                if let Some(val) = value {
                    let cleaned = val.trim().trim_end_matches('.');
                    format!("Decimal::from_str(\"{}\").unwrap()", cleaned)
                } else {
                    format!("Decimal::new(0, {})", pic.decimal_digits)
                }
            } else if let Some(val) = value {
                val.trim().trim_end_matches('.').to_string()
            } else {
                "0".to_string()
            }
        }
    }
}

/// Check if a PIC clause requires the rust_decimal crate.
pub fn requires_decimal(pic: &PicClause) -> bool {
    matches!(pic.pic_type, PicType::Numeric) && pic.decimal_digits > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pic(pic_type: PicType, int_d: u32, dec_d: u32, signed: bool) -> PicClause {
        PicClause {
            raw: String::new(),
            pic_type,
            integer_digits: int_d,
            decimal_digits: dec_d,
            signed,
            total_size: int_d + dec_d,
        }
    }

    #[test]
    fn test_numeric_with_decimal_uses_decimal() {
        let pic = make_pic(PicType::Numeric, 7, 2, false);
        assert_eq!(rust_type_for_pic(&pic), "Decimal");
        assert!(requires_decimal(&pic));
    }

    #[test]
    fn test_numeric_integer_uses_u32() {
        let pic = make_pic(PicType::Numeric, 7, 0, false);
        assert_eq!(rust_type_for_pic(&pic), "u32");
        assert!(!requires_decimal(&pic));
    }

    #[test]
    fn test_signed_numeric_uses_i32() {
        let pic = make_pic(PicType::Numeric, 5, 0, true);
        assert_eq!(rust_type_for_pic(&pic), "i32");
    }

    #[test]
    fn test_alphanumeric_uses_string() {
        let pic = make_pic(PicType::Alphanumeric, 0, 0, false);
        assert_eq!(rust_type_for_pic(&pic), "String");
    }

    #[test]
    fn test_single_digit_uses_u8() {
        let pic = make_pic(PicType::Numeric, 1, 0, false);
        assert_eq!(rust_type_for_pic(&pic), "u8");
    }

    #[test]
    fn test_default_decimal_value() {
        let pic = make_pic(PicType::Numeric, 7, 2, false);
        let default = rust_default_for_pic(&pic, None);
        assert_eq!(default, "Decimal::new(0, 2)");
    }

    #[test]
    fn test_default_string_value() {
        let pic = PicClause {
            raw: "X(30)".to_string(),
            pic_type: PicType::Alphanumeric,
            integer_digits: 0,
            decimal_digits: 0,
            signed: false,
            total_size: 30,
        };
        let default = rust_default_for_pic(&pic, None);
        assert_eq!(default, "String::with_capacity(30)");
    }
}
