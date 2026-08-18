//! WCAG contrast gate for the bundled design tokens (Phase 5 PR40, findings U46-U48).
//!
//! A pure-Rust port of the WCAG 2.x relative-luminance / contrast-ratio formula (no deps). The token
//! hexes below are copied from `src/tokens.css`; if a token is retuned, update it here and this gate
//! proves the accessibility floor still holds. Text needs >=4.5:1; a chip's colored text is measured
//! against its own 12%-tint background (`color-mix(in srgb, hue 12%, transparent)` composited over
//! the panel behind it); confidence dots are non-text and need >=3:1 against the surface.

mod wcag;

use wcag::{contrast, rgb};

/// Alpha-composites `fg` over opaque `bg` at `alpha` (0..1). Models a `color-mix(.. transparent)` wash.
/// Only `chip_bg` below needs this, so it stays local instead of moving into `wcag` — a shared
/// helper only one consumer calls is dead code in every other consumer.
fn composite(fg: (f64, f64, f64), bg: (f64, f64, f64), alpha: f64) -> (f64, f64, f64) {
    let mix = |f: f64, b: f64| f * alpha + b * (1.0 - alpha);
    (mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}

/// Effective background of a chip: the hue washed over the panel behind it at the 12% tint the CSS uses.
fn chip_bg(hue: &str, panel: &str) -> (f64, f64, f64) {
    composite(rgb(hue), rgb(panel), 0.12)
}

const AA_TEXT: f64 = 4.5;
const AA_NON_TEXT: f64 = 3.0;

// Surfaces the tokens are measured against. `--faint` text renders across the base page and secondary
// panels, so it is held to both --bg and --panel-2 (matching its tokens.css comment). Chips (.ev/.resn)
// only ever render inside cards and table cells, whose surface is --panel, so their 12%-tint background
// composites over --panel; confidence dots likewise sit in cards, and also clear the 3:1 floor against
// the raw page --bg. --panel-2 is the transient table-row-hover surface, where the same-hue chip border
// and the always-present confidence label keep the information legible (U48).
// ---- Dark theme surfaces ----
const DARK_BG: &str = "#0f1419";
const DARK_PANEL: &str = "#1a2129";
const DARK_PANEL_2: &str = "#222b35";
// ---- Light theme surfaces ----
const LIGHT_BG: &str = "#f6f8fa";
const LIGHT_PANEL: &str = "#ffffff";
const LIGHT_PANEL_2: &str = "#f0f3f6";

#[test]
fn faint_text_clears_aa_in_both_themes() {
    for surface in [DARK_BG, DARK_PANEL_2] {
        let ratio = contrast(rgb("#8c959e"), rgb(surface));
        assert!(ratio >= AA_TEXT, "dark --faint on {surface}: {ratio:.2} < {AA_TEXT}");
    }
    for surface in [LIGHT_BG, LIGHT_PANEL_2] {
        let ratio = contrast(rgb("#636c75"), rgb(surface));
        assert!(ratio >= AA_TEXT, "light --faint on {surface}: {ratio:.2} < {AA_TEXT}");
    }
}

#[test]
fn evidence_axis_chip_text_clears_aa_in_both_themes() {
    // (hue token) dark values live in :root; light values are the [data-theme="light"] overrides.
    let dark = ["#6cb6ff", "#b27ff0", "#4bb1b1"]; // ev-source / ev-info / ev-evidence
    let light = ["#416d99", "#7d58aa", "#317373"];
    for hue in dark {
        let ratio = contrast(rgb(hue), chip_bg(hue, DARK_PANEL));
        assert!(
            ratio >= AA_TEXT,
            "dark .ev {hue} on {DARK_PANEL} tint: {ratio:.2} < {AA_TEXT}"
        );
    }
    for hue in light {
        let ratio = contrast(rgb(hue), chip_bg(hue, LIGHT_PANEL));
        assert!(
            ratio >= AA_TEXT,
            "light .ev {hue} on {LIGHT_PANEL} tint: {ratio:.2} < {AA_TEXT}"
        );
    }
}

#[test]
fn restriction_chip_text_clears_aa_in_both_themes() {
    let dark = ["#e28e53", "#ed857f", "#bb8ef2"]; // resn confidential / locked / privacy
    let light = ["#8b542e", "#a93d38", "#74529e"];
    for hue in dark {
        let ratio = contrast(rgb(hue), chip_bg(hue, DARK_PANEL));
        assert!(
            ratio >= AA_TEXT,
            "dark .resn {hue} on {DARK_PANEL} tint: {ratio:.2} < {AA_TEXT}"
        );
    }
    for hue in light {
        let ratio = contrast(rgb(hue), chip_bg(hue, LIGHT_PANEL));
        assert!(
            ratio >= AA_TEXT,
            "light .resn {hue} on {LIGHT_PANEL} tint: {ratio:.2} < {AA_TEXT}"
        );
    }
}

#[test]
fn light_confidence_dots_clear_non_text_contrast() {
    // U48: the mockup's treatment darkens the light-theme dot fills so each solid dot clears the 3:1
    // non-text floor against the surface it sits on (no border ring in the mockup CSS).
    let dots = ["#e5534b", "#c17540", "#b58900", "#5a9636", "#2a995f"]; // very-low..very-high
    for dot in dots {
        for surface in [LIGHT_BG, LIGHT_PANEL] {
            let ratio = contrast(rgb(dot), rgb(surface));
            assert!(
                ratio >= AA_NON_TEXT,
                "light .conf dot {dot} on {surface}: {ratio:.2} < {AA_NON_TEXT}"
            );
        }
    }
}
