//! WCAG 2.x contrast ratio, used to refuse unreadable theme colours.

/// "#rrggbb" or "#rgb", case-insensitive.
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.trim().strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let ch = |i: usize, n: usize| u8::from_str_radix(&hex[i..i + n], 16).ok();
    match hex.len() {
        3 => Some((ch(0, 1)? * 17, ch(1, 1)? * 17, ch(2, 1)? * 17)),
        6 => Some((ch(0, 2)?, ch(2, 2)?, ch(4, 2)?)),
        _ => None,
    }
}

/// Relative luminance, 0 (black) to 1 (white).
pub fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let lin = |v: u8| {
        let c = v as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// Contrast ratio between two colours, 1 to 21.
pub fn ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_white_black_ratio() {
        let white = (255u8, 255u8, 255u8);
        let black = (0u8, 0u8, 0u8);
        let ratio = ratio(white, black);
        // Expected: (1.0 + 0.05) / (0.0 + 0.05) = 1.05 / 0.05 = 21.0
        assert!((ratio - 21.0).abs() < 0.01);
    }

    #[test]
    fn test_5c398e_vs_white() {
        let color = (0x5c, 0x39, 0x8e);
        let white = (255u8, 255u8, 255u8);
        let ratio = ratio(color, white);
        // brand/palette.md: white on violet 8.64:1
        assert!((ratio - 8.64).abs() < 0.05, "{ratio}");
    }

    #[test]
    fn test_f3941f_vs_white() {
        let color = (0xf3, 0x94, 0x1f);
        let white = (255u8, 255u8, 255u8);
        let ratio = ratio(color, white);
        // Check below 3.0
        assert!(ratio < 3.0);
    }

    #[test]
    fn test_parse_hex_fff() {
        let result = parse_hex("#FFF");
        assert_eq!(result, Some((255u8, 255u8, 255u8)));
    }

    #[test]
    fn test_parse_hex_red() {
        let result = parse_hex("red");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_hex_12345() {
        let result = parse_hex("#12345");
        assert_eq!(result, None);
    }
}
