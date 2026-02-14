use crate::format::format_compact;

pub fn format_hms(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "--:--:--".to_string();
    }
    let total = seconds.round() as i64;
    let sec = total.rem_euclid(60);
    let min_total = total.div_euclid(60);
    let min = min_total.rem_euclid(60);
    let hour = min_total.div_euclid(60);
    format!("{hour:02}:{min:02}:{sec:02}")
}

pub fn format_time_or_number(value: f32) -> String {
    if !value.is_finite() {
        return format_compact(value);
    }
    let abs = value.abs();
    if abs >= 60.0 {
        return format_hms(value as f64);
    }
    format_compact(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hms_formats_expected() {
        assert_eq!(format_hms(3661.0), "01:01:01");
    }

    #[test]
    fn time_or_number_uses_hms_for_minute_ranges() {
        assert_eq!(format_time_or_number(61.0), "00:01:01");
        assert_eq!(format_time_or_number(3661.0), "01:01:01");
    }
}
