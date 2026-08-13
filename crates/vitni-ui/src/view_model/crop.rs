//! Framework-free crop-rectangle math (ADR 0017 §9): the pure geometry behind the media viewer's
//! drag-to-draw region tool. The renderer supplies pointer coordinates and the framed image's
//! measured size; these functions turn them into a percent-based [`Rect`] (so the region survives any
//! zoom) and back into the inline CSS the overlay is positioned with. Kept here, unit- and
//! property-tested in isolation, so the renderer's pointer handlers stay thin closures over tested math.

use vitni_app::Rect;

/// The smallest region the tool will commit, in whole percent: a drag narrower or shorter than this
/// on either axis is treated as an accidental click and yields no region.
const MIN_SIZE_PCT: u8 = 1;

/// Builds a percent-based [`Rect`] from a pointer drag.
///
/// `start` and `current` are the drag's endpoints in pixels relative to the framed image's top-left;
/// `bounds` is the frame's measured `(width, height)` in pixels. The result is direction-agnostic
/// (dragging up-left gives the same rectangle as down-right), clamped so the region stays fully
/// within `0..=100`% on both axes, and rounded to whole percent. Returns `None` for a degenerate
/// drag (a zero-area frame, or a region under [`MIN_SIZE_PCT`] on either axis — e.g. a bare click).
#[must_use]
pub fn rect_from_drag(start: (f64, f64), current: (f64, f64), bounds: (f64, f64)) -> Option<Rect> {
    let (width_px, height_px) = bounds;
    if width_px <= 0.0 || height_px <= 0.0 {
        return None;
    }

    let left_px = start.0.min(current.0).clamp(0.0, width_px);
    let right_px = start.0.max(current.0).clamp(0.0, width_px);
    let top_px = start.1.min(current.1).clamp(0.0, height_px);
    let bottom_px = start.1.max(current.1).clamp(0.0, height_px);

    let left = round_pct(left_px / width_px * 100.0);
    let right = round_pct(right_px / width_px * 100.0);
    let top = round_pct(top_px / height_px * 100.0);
    let bottom = round_pct(bottom_px / height_px * 100.0);

    // `round_pct` is monotonic and each edge is clamped to `0..=100`, so `right >= left` and the
    // width can never push `left + width` past 100 — the region stays inside the frame.
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    if width < MIN_SIZE_PCT || height < MIN_SIZE_PCT {
        return None;
    }
    Some(Rect {
        left,
        top,
        width,
        height,
    })
}

/// The inline CSS positioning a percent-based [`Rect`] over its `position:relative` frame — the
/// `.crop-rect`/`.crop-outline` overlay geometry (media.html), zoom-invariant because it is expressed
/// in percentages of the frame rather than pixels.
#[must_use]
pub fn rect_css(rect: &Rect) -> String {
    format!(
        "left:{}%;top:{}%;width:{}%;height:{}%",
        rect.left, rect.top, rect.width, rect.height
    )
}

/// Rounds a percentage already clamped to `0.0..=100.0` to the nearest whole percent as a `u8`.
///
/// Done cast-free (a counting loop over the lossless `u8 -> f64` [`From`] conversion) so the pedantic
/// float-to-int cast lints stay silent; the range is at most 100 steps, negligible for UI math.
fn round_pct(value: f64) -> u8 {
    let rounded = value.round().clamp(0.0, 100.0);
    let mut pct: u8 = 0;
    while pct < 100 && f64::from(pct) < rounded {
        pct += 1;
    }
    pct
}

#[cfg(test)]
mod tests {
    use super::{rect_css, rect_from_drag};
    use proptest::prelude::*;
    use vitni_app::Rect;

    #[test]
    fn a_forward_drag_maps_to_percentages_of_the_frame() {
        // A 40×30 region on a 200×200 frame starting at (20,20): 10% inset, 20%×15% region.
        let rect = rect_from_drag((20.0, 20.0), (60.0, 50.0), (200.0, 200.0)).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 10,
                top: 10,
                width: 20,
                height: 15,
            }
        );
    }

    #[test]
    fn dragging_up_left_gives_the_same_rectangle_as_down_right() {
        let forward = rect_from_drag((20.0, 20.0), (60.0, 50.0), (200.0, 200.0));
        let backward = rect_from_drag((60.0, 50.0), (20.0, 20.0), (200.0, 200.0));
        assert_eq!(forward, backward);
    }

    #[test]
    fn a_drag_beyond_the_frame_is_clamped_inside() {
        let rect = rect_from_drag((-50.0, -50.0), (400.0, 400.0), (200.0, 200.0)).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 0,
                top: 0,
                width: 100,
                height: 100,
            }
        );
    }

    #[test]
    fn a_bare_click_is_degenerate() {
        assert_eq!(rect_from_drag((30.0, 30.0), (30.0, 30.0), (200.0, 200.0)), None);
    }

    #[test]
    fn a_sub_percent_drag_is_degenerate() {
        // A drag that rounds to the same whole percent on both edges has zero width/height.
        assert_eq!(rect_from_drag((10.0, 10.0), (10.4, 10.4), (200.0, 200.0)), None);
    }

    #[test]
    fn a_zero_area_frame_yields_no_region() {
        assert_eq!(rect_from_drag((0.0, 0.0), (10.0, 10.0), (0.0, 200.0)), None);
        assert_eq!(rect_from_drag((0.0, 0.0), (10.0, 10.0), (200.0, 0.0)), None);
    }

    #[test]
    fn css_renders_percent_geometry() {
        let rect = Rect {
            left: 22,
            top: 18,
            width: 39,
            height: 26,
        };
        assert_eq!(rect_css(&rect), "left:22%;top:18%;width:39%;height:26%");
    }

    proptest! {
        /// However the drag is oriented and wherever it lands, a committed region is always a valid
        /// percentage rectangle fully inside the frame.
        #[test]
        fn a_committed_region_stays_within_the_frame(
            x0 in -100.0f64..600.0,
            y0 in -100.0f64..600.0,
            x1 in -100.0f64..600.0,
            y1 in -100.0f64..600.0,
            w in 1.0f64..2000.0,
            h in 1.0f64..2000.0,
        ) {
            if let Some(rect) = rect_from_drag((x0, y0), (x1, y1), (w, h)) {
                prop_assert!(rect.width >= 1 && rect.height >= 1);
                prop_assert!(u16::from(rect.left) + u16::from(rect.width) <= 100);
                prop_assert!(u16::from(rect.top) + u16::from(rect.height) <= 100);
            }
        }

        /// The tool is direction-agnostic: swapping the drag's endpoints never changes the region.
        #[test]
        fn direction_does_not_change_the_region(
            x0 in -100.0f64..600.0,
            y0 in -100.0f64..600.0,
            x1 in -100.0f64..600.0,
            y1 in -100.0f64..600.0,
            w in 1.0f64..2000.0,
            h in 1.0f64..2000.0,
        ) {
            let forward = rect_from_drag((x0, y0), (x1, y1), (w, h));
            let backward = rect_from_drag((x1, y1), (x0, y0), (w, h));
            prop_assert_eq!(forward, backward);
        }
    }
}
