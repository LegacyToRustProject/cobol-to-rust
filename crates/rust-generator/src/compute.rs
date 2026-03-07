/// COMPUTE statement code generation.
///
/// COBOL COMPUTE has two modes:
/// - Default (no ROUNDED): truncate result to target PIC precision
/// - ROUNDED: round result to target PIC precision using half-up rounding
///
/// With `rust_decimal`:
/// - Truncation: `result.trunc_with_scale(decimal_places)`
/// - Rounding: `result.round_dp(decimal_places)`
use rust_decimal::{Decimal, RoundingStrategy};

/// Generate the Rust assignment expression for a COMPUTE statement.
///
/// # Parameters
/// - `target`: Rust variable name (snake_case)
/// - `expression`: Rust expression (already translated from COBOL arithmetic)
/// - `decimal_places`: target precision (from the PIC clause of `target`)
/// - `rounded`: whether ROUNDED keyword was present
pub fn generate_compute_assignment(
    target: &str,
    expression: &str,
    decimal_places: u32,
    rounded: bool,
) -> String {
    if decimal_places == 0 {
        // Integer target: no decimal scaling needed
        format!("{} = ({}) as i64;", target, expression)
    } else if rounded {
        // COBOL ROUNDED uses half-up (MidpointAwayFromZero), not banker's rounding
        format!(
            "{} = ({}).round_dp_with_strategy({}, rust_decimal::RoundingStrategy::MidpointAwayFromZero);",
            target, expression, decimal_places
        )
    } else {
        // COBOL default: truncate (not round)
        format!(
            "{} = ({}).trunc_with_scale({});",
            target, expression, decimal_places
        )
    }
}

/// Simulate COBOL COMPUTE truncation behavior for `Decimal`.
///
/// COBOL truncates (does not round) intermediate results to the target PIC precision
/// unless ROUNDED is specified.
pub fn cobol_truncate(value: Decimal, decimal_places: u32) -> Decimal {
    let factor = Decimal::from(10u64.pow(decimal_places));
    let scaled = (value * factor).trunc();
    scaled / factor
}

/// Simulate COBOL COMPUTE ROUNDED behavior for `Decimal`.
///
/// Uses half-up (MidpointAwayFromZero) rounding — COBOL's default rounding mode.
/// Note: `Decimal::round_dp` uses banker's rounding; this function uses arithmetic rounding.
pub fn cobol_round(value: Decimal, decimal_places: u32) -> Decimal {
    value.round_dp_with_strategy(decimal_places, RoundingStrategy::MidpointAwayFromZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn test_generate_compute_no_rounded_decimal() {
        let code = generate_compute_assignment("ws_result", "ws_a / ws_b", 2, false);
        assert!(
            code.contains("trunc_with_scale(2)"),
            "truncation should use trunc_with_scale: {}",
            code
        );
        assert!(code.contains("ws_result"), "target should appear");
    }

    #[test]
    fn test_generate_compute_rounded_decimal() {
        let code = generate_compute_assignment("ws_result", "ws_a / ws_b", 2, true);
        assert!(
            code.contains("MidpointAwayFromZero"),
            "rounding should use half-up strategy: {}",
            code
        );
        assert!(!code.contains("trunc"), "ROUNDED should not truncate");
    }

    #[test]
    fn test_generate_compute_integer_target() {
        let code = generate_compute_assignment("ws_count", "ws_a + ws_b", 0, false);
        assert!(
            code.contains("as i64"),
            "integer target should cast: {}",
            code
        );
    }

    #[test]
    fn test_cobol_truncate_not_round() {
        // 1/3 = 0.333... → truncate to 2dp = 0.33
        let result = cobol_truncate(d("1") / d("3"), 2);
        assert_eq!(result, d("0.33"), "truncation: 1/3 with 2dp should be 0.33");
    }

    #[test]
    fn test_cobol_truncate_downward() {
        // 2/3 = 0.666... → truncate to 2dp = 0.66 (not 0.67)
        let result = cobol_truncate(d("2") / d("3"), 2);
        assert_eq!(
            result,
            d("0.66"),
            "truncation: 2/3 with 2dp should be 0.66 not 0.67"
        );
    }

    #[test]
    fn test_cobol_round_up() {
        // 2/3 = 0.666... → ROUNDED to 2dp = 0.67 (half-up rounding)
        let result = cobol_round(d("2") / d("3"), 2);
        assert_eq!(
            result,
            d("0.67"),
            "rounding: 2/3 with 2dp ROUNDED should be 0.67"
        );
    }

    #[test]
    fn test_cobol_round_half_up() {
        // 0.125 → ROUNDED to 2dp = 0.13 (half-up)
        let result = cobol_round(d("0.125"), 2);
        assert_eq!(result, d("0.13"), "0.125 rounded to 2dp should be 0.13");
    }

    #[test]
    fn test_cobol_truncate_negative() {
        // -2/3 = -0.666... → truncate to 2dp = -0.66 (toward zero)
        let result = cobol_truncate(d("-2") / d("3"), 2);
        assert_eq!(
            result,
            d("-0.66"),
            "negative truncation should go toward zero"
        );
    }

    #[test]
    fn test_generate_compute_4dp_rounded() {
        let code = generate_compute_assignment("ws_rate", "ws_amount * ws_factor", 4, true);
        assert!(
            code.contains("round_dp_with_strategy(4"),
            "4dp ROUNDED should use round_dp_with_strategy: {}",
            code
        );
    }
}
