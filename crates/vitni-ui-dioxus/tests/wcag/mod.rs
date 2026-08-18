//! WCAG 2.x relative-luminance / contrast-ratio helpers (Phase 5 PR40, findings U46-U48).
//!
//! A pure-Rust port of the WCAG formula (no deps), shared by every CSS gate under
//! `crates/vitni-ui-dioxus/tests/` that needs to compare two resolved colors. `composite` (the
//! alpha-blend used for a chip's 12%-tint background) stays local to `contrast.rs`, the only
//! consumer that needs it — every function here is used by every consumer.

/// Parses a `#rrggbb` string into linear-independent 0-255 channels.
pub fn rgb(hex: &str) -> (f64, f64, f64) {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    (f64::from(r), f64::from(g), f64::from(b))
}

/// sRGB 8-bit channel -> linear-light component (WCAG 2.x).
pub fn linearize(channel: f64) -> f64 {
    let c = channel / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an sRGB color.
pub fn luminance(color: (f64, f64, f64)) -> f64 {
    let (r, g, b) = color;
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG contrast ratio between two opaque colors (>=1.0).
pub fn contrast(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let la = luminance(a);
    let lb = luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
