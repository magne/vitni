//! Framework-free crop-rectangle math (ADR 0017 §9): the pure geometry behind the media viewer's
//! region tool. The renderer supplies pointer coordinates and the framed image's measured size; these
//! functions turn them into a percent-based [`Rect`] (so the region survives any zoom) and back into
//! the inline CSS the overlay is positioned with. Kept here, unit- and property-tested in isolation, so
//! the renderer's pointer handlers stay thin closures over tested math.
//!
//! Three gestures, one geometry: [`rect_from_drag`] draws a region from scratch, [`rect_resized`] drags
//! one corner grip with the opposite corner anchored, and [`rect_moved`] translates the whole region at
//! a fixed size. [`rect_contains`] is the hit-test that tells the second and third apart from the first.

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

/// Which corner grip of a region is being dragged (`media.html`'s four `.crop-handle` spans). The
/// opposite corner is the anchor a resize pivots on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropCorner {
    /// The top-left grip: moves the region's origin, anchoring the bottom-right.
    NorthWest,
    /// The top-right grip: moves the top edge and the right edge.
    NorthEast,
    /// The bottom-left grip: moves the left edge and the bottom edge.
    SouthWest,
    /// The bottom-right grip: moves the far corner, anchoring the origin.
    SouthEast,
}

impl CropCorner {
    /// Whether this corner owns the region's left edge (rather than its right).
    const fn holds_left(self) -> bool {
        match self {
            CropCorner::NorthWest | CropCorner::SouthWest => true,
            CropCorner::NorthEast | CropCorner::SouthEast => false,
        }
    }

    /// Whether this corner owns the region's top edge (rather than its bottom).
    const fn holds_top(self) -> bool {
        match self {
            CropCorner::NorthWest | CropCorner::NorthEast => true,
            CropCorner::SouthWest | CropCorner::SouthEast => false,
        }
    }
}

/// Resizes `rect` by dragging one corner to `to`, in pixels relative to the framed image's top-left.
///
/// The dragged corner follows the pointer and **the opposite corner stays anchored**; every edge is
/// clamped to `0..=100`%. A drag that would invert the rectangle — pulling a corner past its anchor —
/// **stops at [`MIN_SIZE_PCT`]** rather than flipping the anchor, so a grip keeps its identity for the
/// whole gesture instead of turning into the opposite grip under the pointer. Returns `None` for a
/// zero-area frame, like [`rect_from_drag`].
#[must_use]
pub fn rect_resized(rect: Rect, corner: CropCorner, to: (f64, f64), bounds: (f64, f64)) -> Option<Rect> {
    let (width_px, height_px) = bounds;
    if width_px <= 0.0 || height_px <= 0.0 {
        return None;
    }
    let dragged_x = round_pct(to.0.clamp(0.0, width_px) / width_px * 100.0);
    let dragged_y = round_pct(to.1.clamp(0.0, height_px) / height_px * 100.0);
    let right = rect.left.saturating_add(rect.width);
    let bottom = rect.top.saturating_add(rect.height);

    let (left, width) = resize_axis(dragged_x, rect.left, right, corner.holds_left());
    let (top, height) = resize_axis(dragged_y, rect.top, bottom, corner.holds_top());
    Some(Rect {
        left,
        top,
        width,
        height,
    })
}

/// One axis of a corner resize: `dragged` is the pointer's percent on this axis, `near`/`far` the
/// region's current edges, and `moves_near` whether the dragged corner owns the near edge. Returns the
/// axis's `(origin, size)`, never smaller than [`MIN_SIZE_PCT`].
fn resize_axis(dragged: u8, near: u8, far: u8, moves_near: bool) -> (u8, u8) {
    if moves_near {
        // The far edge is the anchor; the origin cannot reach it.
        let origin = dragged.min(far.saturating_sub(MIN_SIZE_PCT));
        (origin, far.saturating_sub(origin).max(MIN_SIZE_PCT))
    } else {
        // The near edge is the anchor; the far edge cannot reach back past it.
        let end = dragged.max(near.saturating_add(MIN_SIZE_PCT).min(100));
        (near, end.saturating_sub(near).max(MIN_SIZE_PCT))
    }
}

/// Translates `rect` by a pixel `delta` relative to the framed image, keeping its size.
///
/// A move that would push the region past an edge **stops at the edge at full size** rather than
/// shrinking against it — the size is the one thing a move must not change. Returns `None` for a
/// zero-area frame.
#[must_use]
pub fn rect_moved(rect: Rect, delta: (f64, f64), bounds: (f64, f64)) -> Option<Rect> {
    let (width_px, height_px) = bounds;
    if width_px <= 0.0 || height_px <= 0.0 {
        return None;
    }
    Some(Rect {
        left: shifted(rect.left, rect.width, delta.0 / width_px * 100.0),
        top: shifted(rect.top, rect.height, delta.1 / height_px * 100.0),
        width: rect.width,
        height: rect.height,
    })
}

/// One axis of a move: `origin` shifted by `delta` percent, clamped so `origin + size` stays within
/// 100 — so the region stops against an edge at full size.
fn shifted(origin: u8, size: u8, delta: f64) -> u8 {
    let limit = 100_u8.saturating_sub(size);
    round_pct(f64::from(origin) + delta).min(limit)
}

/// Whether `point` (pixels relative to the framed image) lies inside `rect` — the hit-test that tells
/// a press on the region's interior (a move) from a press on empty frame (a fresh draw). Edges count as
/// inside; a zero-area frame contains nothing.
#[must_use]
pub fn rect_contains(rect: Rect, point: (f64, f64), bounds: (f64, f64)) -> bool {
    let (width_px, height_px) = bounds;
    if width_px <= 0.0 || height_px <= 0.0 {
        return false;
    }
    let x = point.0 / width_px * 100.0;
    let y = point.1 / height_px * 100.0;
    x >= f64::from(rect.left)
        && x <= f64::from(rect.left.saturating_add(rect.width))
        && y >= f64::from(rect.top)
        && y <= f64::from(rect.top.saturating_add(rect.height))
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
mod adjust_tests {
    use super::{CropCorner, MIN_SIZE_PCT, rect_contains, rect_moved, rect_resized};
    use proptest::prelude::*;
    use vitni_app::Rect;

    const FRAME: (f64, f64) = (200.0, 200.0);

    /// A 20%x15% region inset 10% from the top-left — on a 200px frame, pixels 20..60 by 20..50.
    fn region() -> Rect {
        Rect {
            left: 10,
            top: 10,
            width: 20,
            height: 15,
        }
    }

    #[test]
    fn dragging_the_south_east_handle_moves_that_corner_and_anchors_the_other() {
        let rect = rect_resized(region(), CropCorner::SouthEast, (100.0, 100.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 10,
                top: 10,
                width: 40,
                height: 40
            }
        );
    }

    #[test]
    fn dragging_the_north_west_handle_moves_the_origin_and_keeps_the_far_corner() {
        // The far corner is at 30%,25%; pulling the origin to 0,0 must keep it there.
        let rect = rect_resized(region(), CropCorner::NorthWest, (0.0, 0.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 0,
                top: 0,
                width: 30,
                height: 25
            }
        );
    }

    #[test]
    fn dragging_the_north_east_handle_moves_one_edge_on_each_axis() {
        let rect = rect_resized(region(), CropCorner::NorthEast, (140.0, 0.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 10,
                top: 0,
                width: 60,
                height: 25
            }
        );
    }

    #[test]
    fn dragging_the_south_west_handle_moves_the_other_pair() {
        let rect = rect_resized(region(), CropCorner::SouthWest, (0.0, 140.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 0,
                top: 10,
                width: 30,
                height: 60
            }
        );
    }

    #[test]
    fn a_resize_past_the_frame_stops_at_the_edge() {
        let rect = rect_resized(region(), CropCorner::SouthEast, (400.0, 400.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 10,
                top: 10,
                width: 90,
                height: 90
            }
        );
    }

    #[test]
    fn a_resize_that_would_invert_the_rectangle_clamps_at_the_minimum() {
        // Chosen deliberately over flipping the anchor: a handle keeps its identity for the whole
        // gesture, so an `nw` grip never becomes the `se` grip under the pointer mid-drag.
        let rect = rect_resized(region(), CropCorner::SouthEast, (0.0, 0.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 10,
                top: 10,
                width: MIN_SIZE_PCT,
                height: MIN_SIZE_PCT
            }
        );
    }

    #[test]
    fn an_inverting_origin_drag_clamps_against_the_anchored_far_corner() {
        // The far corner is 30%,25%; the origin cannot pass it, so it stops one percent short.
        let rect = rect_resized(region(), CropCorner::NorthWest, (400.0, 400.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 30 - MIN_SIZE_PCT,
                top: 25 - MIN_SIZE_PCT,
                width: MIN_SIZE_PCT,
                height: MIN_SIZE_PCT
            }
        );
    }

    #[test]
    fn a_move_translates_the_region_and_keeps_its_size() {
        let rect = rect_moved(region(), (20.0, 20.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 20,
                top: 20,
                width: 20,
                height: 15
            }
        );
    }

    #[test]
    fn a_move_past_the_far_edge_stops_there_at_full_size() {
        // Not shrunk against the edge: the region keeps 20x15 and its right/bottom sit on 100%.
        let rect = rect_moved(region(), (1000.0, 1000.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 80,
                top: 85,
                width: 20,
                height: 15
            }
        );
    }

    #[test]
    fn a_move_past_the_near_edge_stops_at_zero_at_full_size() {
        let rect = rect_moved(region(), (-1000.0, -1000.0), FRAME).expect("a region");
        assert_eq!(
            rect,
            Rect {
                left: 0,
                top: 0,
                width: 20,
                height: 15
            }
        );
    }

    #[test]
    fn a_zero_area_frame_adjusts_nothing() {
        assert_eq!(
            rect_resized(region(), CropCorner::SouthEast, (10.0, 10.0), (0.0, 200.0)),
            None
        );
        assert_eq!(rect_moved(region(), (10.0, 10.0), (200.0, 0.0)), None);
    }

    #[test]
    fn the_interior_is_what_a_move_grabs_and_the_outside_is_not() {
        // Pixels 20..60 by 20..50 on a 200px frame.
        assert!(rect_contains(region(), (40.0, 35.0), FRAME));
        assert!(
            rect_contains(region(), (20.0, 20.0), FRAME),
            "the top-left corner counts as inside"
        );
        assert!(!rect_contains(region(), (19.0, 35.0), FRAME));
        assert!(!rect_contains(region(), (40.0, 51.0), FRAME));
        assert!(
            !rect_contains(region(), (40.0, 35.0), (0.0, 0.0)),
            "a zero-area frame contains nothing"
        );
    }

    #[test]
    fn an_adjustment_reads_the_same_percentages_at_any_zoom() {
        // The geometry is percent-based (ADR 0017 §GUI), so the same gesture scaled with the frame must
        // produce the same region — that is what keeps a region valid across zoom steps.
        let small = rect_resized(region(), CropCorner::SouthEast, (100.0, 100.0), (200.0, 200.0));
        let large = rect_resized(region(), CropCorner::SouthEast, (200.0, 200.0), (400.0, 400.0));
        assert_eq!(small, large);
        let moved_small = rect_moved(region(), (20.0, 20.0), (200.0, 200.0));
        let moved_large = rect_moved(region(), (40.0, 40.0), (400.0, 400.0));
        assert_eq!(moved_small, moved_large);
    }

    proptest! {
        /// However a corner is dragged, the result is a valid percentage rectangle inside the frame that
        /// never collapses below the minimum.
        #[test]
        fn a_resized_region_stays_valid(
            left in 0u8..=99, top in 0u8..=99, w in 1u8..=100, h in 1u8..=100,
            x in -400.0f64..800.0, y in -400.0f64..800.0,
            fw in 1.0f64..2000.0, fh in 1.0f64..2000.0,
            corner in 0usize..4,
        ) {
            let width = w.min(100 - left).max(MIN_SIZE_PCT);
            let height = h.min(100 - top).max(MIN_SIZE_PCT);
            let orig = Rect { left, top, width, height };
            let corner = [
                CropCorner::NorthWest, CropCorner::NorthEast, CropCorner::SouthWest, CropCorner::SouthEast,
            ][corner];
            if let Some(rect) = rect_resized(orig, corner, (x, y), (fw, fh)) {
                prop_assert!(rect.width >= MIN_SIZE_PCT && rect.height >= MIN_SIZE_PCT);
                prop_assert!(u16::from(rect.left) + u16::from(rect.width) <= 100);
                prop_assert!(u16::from(rect.top) + u16::from(rect.height) <= 100);
            }
        }

        /// A move never changes the region's size, however far it is pushed.
        #[test]
        fn a_moved_region_keeps_its_size_and_stays_inside(
            left in 0u8..=99, top in 0u8..=99, w in 1u8..=100, h in 1u8..=100,
            dx in -4000.0f64..4000.0, dy in -4000.0f64..4000.0,
            fw in 1.0f64..2000.0, fh in 1.0f64..2000.0,
        ) {
            let width = w.min(100 - left).max(MIN_SIZE_PCT);
            let height = h.min(100 - top).max(MIN_SIZE_PCT);
            let orig = Rect { left, top, width, height };
            if let Some(rect) = rect_moved(orig, (dx, dy), (fw, fh)) {
                prop_assert_eq!(rect.width, orig.width);
                prop_assert_eq!(rect.height, orig.height);
                prop_assert!(u16::from(rect.left) + u16::from(rect.width) <= 100);
                prop_assert!(u16::from(rect.top) + u16::from(rect.height) <= 100);
            }
        }
    }
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
