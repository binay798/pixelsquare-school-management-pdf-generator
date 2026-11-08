//! Formatting helpers shared by the document models.
//!
//! The templates never format numbers or dates themselves: every value that is
//! shown to a reader is prepared here so all documents of the service look the
//! same (thousands separators, two decimals for money, `08 Nov 2026` for dates).

use chrono::NaiveDate;

/// Format that dates are accepted in.
pub(crate) const DATE_FORMAT: &str = "%Y-%m-%d";

/// Format that dates are rendered in.
pub(crate) const DISPLAY_DATE_FORMAT: &str = "%d %b %Y";

/// Rounds a value to two decimals.
pub(crate) fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Formats a number with a fixed amount of decimals and thousands separators.
pub(crate) fn format_number(value: f64, decimals: usize) -> String {
    let factor = 10_i128.pow(decimals as u32);
    let scaled = (value.abs() * factor as f64).round() as i128;
    let whole = scaled / factor;
    let fraction = scaled % factor;

    let digits = whole.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    // `-0.00` is never useful, so the sign is dropped when nothing is left.
    let sign = if value < 0.0 && scaled != 0 { "-" } else { "" };

    if decimals == 0 {
        format!("{sign}{grouped}")
    } else {
        let fraction = format!("{fraction:0>width$}", width = decimals);
        format!("{sign}{grouped}.{fraction}")
    }
}

/// Formats a monetary value with two decimals and thousands separators.
pub(crate) fn format_money(value: f64) -> String {
    format_number(value, 2)
}

/// Formats a quantity without unnecessary trailing decimals.
pub(crate) fn format_quantity(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value.trunc() as i64)
    } else {
        let text = format!("{:.2}", round2(value));
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Formats a percentage value, e.g. `87.5%`.
pub(crate) fn format_percentage(value: f64) -> String {
    format!("{}%", format_quantity(round2(value)))
}

/// Formats an ISO date as e.g. `08 Nov 2026`.
pub(crate) fn format_date(value: &str) -> String {
    NaiveDate::parse_from_str(value, DATE_FORMAT)
        .map(|date| date.format(DISPLAY_DATE_FORMAT).to_string())
        .unwrap_or_else(|_| value.to_string())
}
