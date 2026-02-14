pub fn format_fixed(value: f32, decimals: usize) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".to_string()
        } else if value.is_sign_positive() {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        };
    }
    format!("{value:.decimals$}")
}

pub fn format_compact(value: f32) -> String {
    if !value.is_finite() {
        return format_fixed(value, 0);
    }
    let abs = value.abs();
    if abs >= 1_000_000_000.0 {
        return format_with_suffix(value / 1_000_000_000.0, "B");
    }
    if abs >= 1_000_000.0 {
        return format_with_suffix(value / 1_000_000.0, "M");
    }
    if abs >= 1_000.0 {
        return format_with_suffix(value / 1_000.0, "K");
    }

    trim_trailing_zeroes(format!("{value:.3}"))
}

fn format_with_suffix(value: f32, suffix: &str) -> String {
    format!("{}{}", trim_trailing_zeroes(format!("{value:.2}")), suffix)
}

fn trim_trailing_zeroes(mut s: String) -> String {
    if let Some(dot) = s.find('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.len() == dot + 1 {
            s.pop();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_uses_suffixes() {
        assert_eq!(format_compact(12_400.0), "12.4K");
        assert_eq!(format_compact(2_000_000.0), "2M");
    }

    #[test]
    fn compact_keeps_useful_precision_for_close_values() {
        assert_eq!(format_compact(1_210.0), "1.21K");
        assert_eq!(format_compact(1_290.0), "1.29K");
    }
}
