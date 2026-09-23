//! Human-readable sizes, decimal units like Finder and GNOME Files.

const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];

/// "Empty", "512 bytes", "20 KB", "1.5 MB", "110 MB": one decimal below 10,
/// none above, and no trailing ".0".
pub fn size(bytes: u64) -> String {
    if bytes == 0 {
        return "Empty".into();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    // Divide while the number would print as 1000 or more, so 999,999 bytes
    // reads "1 MB" rather than "1000 KB".
    while value.round() >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        return format!("{bytes} bytes");
    }
    let text = if value < 9.95 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };
    format!("{} {}", text.trim_end_matches(".0"), UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::size;

    #[test]
    fn formats_like_finder() {
        assert_eq!(size(0), "Empty");
        assert_eq!(size(512), "512 bytes");
        assert_eq!(size(20_000), "20 KB");
        assert_eq!(size(1_500_000), "1.5 MB");
        assert_eq!(size(110_000_000), "110 MB");
        assert_eq!(size(2_340_000_000), "2.3 GB");
    }

    #[test]
    fn round_numbers_have_no_trailing_zero() {
        assert_eq!(size(1_000_000), "1 MB");
        assert_eq!(size(9_960_000), "10 MB");
    }

    #[test]
    fn never_shows_a_thousand_of_a_unit() {
        assert_eq!(size(999_999), "1 MB");
        assert_eq!(size(999_400), "999 KB");
    }
}
