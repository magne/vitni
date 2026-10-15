//! The windowing arithmetic of a long list (#590): which rows a renderer mounts out of a list too long
//! to mount whole. Pure index math over the list's length, its scroll offset and its roving tab stop,
//! so it is testable without a renderer; the renderer owns the pixels and asks for row indices.

/// How many consecutive rows a window mounts — enough to fill the tallest list column with
/// [`OVERSCAN`] rows to spare above and below.
pub const WINDOW_ROWS: usize = 64;

/// Rows mounted above the first row in view, so a short scroll or an ↑ never reaches an unmounted
/// row before the next scroll event moves the window.
const OVERSCAN: usize = 8;

/// The mounted slice of a long list: rows `first..first + WINDOW_ROWS`, clamped to the list's end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListWindow {
    first: usize,
}

impl ListWindow {
    /// The window for a list of `total` rows scrolled `scroll_top` pixels into a content
    /// `scroll_height` pixels tall: the rows are taken as equally tall, so the row at the top of the
    /// view is `scroll_top / (scroll_height / total)`, and the window starts [`OVERSCAN`] rows above it.
    #[must_use]
    pub fn at_scroll(scroll_top: f64, scroll_height: f64, total: usize) -> Self {
        let Some(last) = total.checked_sub(1) else {
            return Self::default();
        };
        if scroll_top <= 0.0 || scroll_height <= 0.0 {
            return Self::default();
        }
        let row_px = scroll_height / count_f64(total);
        Self {
            first: top_row(scroll_top / row_px, last).saturating_sub(OVERSCAN),
        }
    }

    /// This window moved the least distance that mounts row `index`: unchanged when `index` is
    /// already inside it.
    #[must_use]
    pub fn revealing(self, index: usize) -> Self {
        if index < self.first {
            Self {
                first: index.saturating_sub(OVERSCAN),
            }
        } else if index >= self.first + WINDOW_ROWS {
            Self {
                first: (index + OVERSCAN + 1).saturating_sub(WINDOW_ROWS),
            }
        } else {
            self
        }
    }

    /// The row indices to mount, ascending, for a list of `total` rows whose roving tab stop is row
    /// `stop`: the window's rows plus the stop, so keyboard focus can always enter the list.
    #[must_use]
    pub fn rendered(self, total: usize, stop: usize) -> Vec<usize> {
        let start = self.first.min(total.saturating_sub(WINDOW_ROWS));
        let window = start..total.min(start + WINDOW_ROWS);
        let mut rendered = Vec::with_capacity(WINDOW_ROWS + 1);
        if stop < window.start {
            rendered.push(stop);
        }
        rendered.extend(window.clone());
        if stop >= window.end && stop < total {
            rendered.push(stop);
        }
        rendered
    }
}

/// `count` as an `f64`, through the lossless `u32` conversion — a list longer than `u32::MAX` rows
/// is beyond any screen, so saturating there loses nothing.
fn count_f64(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}

/// The whole row a fractional `row` offset falls in, clamped to `0..=last`.
///
/// Found by bisection over the lossless `usize -> f64` route of [`count_f64`] rather than a float cast,
/// so the pedantic float-to-int cast lints stay silent; at most ~32 steps.
fn top_row(row: f64, last: usize) -> usize {
    let (mut low, mut high) = (0, last);
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if count_f64(mid) <= row {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    low
}

#[cfg(test)]
mod tests {
    use super::{ListWindow, OVERSCAN, WINDOW_ROWS};
    use proptest::prelude::*;

    const ROW_PX: f64 = 49.0;

    fn at_row(row: u32, total: usize) -> ListWindow {
        let height = ROW_PX * f64::from(u32::try_from(total).unwrap_or(u32::MAX));
        ListWindow::at_scroll(ROW_PX * f64::from(row), height, total)
    }

    #[test]
    fn a_fresh_window_mounts_the_head_of_the_list() {
        let rendered = ListWindow::default().rendered(100_000, 0);
        assert_eq!(rendered, (0..WINDOW_ROWS).collect::<Vec<_>>());
    }

    #[test]
    fn a_short_list_mounts_every_row() {
        assert_eq!(ListWindow::default().rendered(3, 0), vec![0, 1, 2]);
        assert_eq!(ListWindow::default().rendered(0, 0), Vec::<usize>::new());
    }

    #[test]
    fn a_scrolled_window_starts_overscan_rows_above_the_top_row() {
        let rendered = at_row(1_000, 100_000).rendered(100_000, 1_000);
        assert_eq!(rendered.first(), Some(&(1_000 - OVERSCAN)));
        assert_eq!(rendered.len(), WINDOW_ROWS);
    }

    #[test]
    fn a_window_scrolled_to_the_end_still_mounts_a_full_window() {
        let rendered = at_row(99_990, 100_000).rendered(100_000, 99_999);
        assert_eq!(rendered.last(), Some(&99_999));
        assert_eq!(rendered.len(), WINDOW_ROWS);
    }

    #[test]
    fn the_tab_stop_is_mounted_outside_the_window() {
        let rendered = at_row(5_000, 100_000).rendered(100_000, 3);
        assert_eq!(rendered.first(), Some(&3));
        assert_eq!(rendered.len(), WINDOW_ROWS + 1);
    }

    #[test]
    fn an_unmeasured_scroll_keeps_the_head() {
        assert_eq!(ListWindow::at_scroll(500.0, 0.0, 100), ListWindow::default());
        assert_eq!(ListWindow::at_scroll(-20.0, 4_900.0, 100), ListWindow::default());
        assert_eq!(ListWindow::at_scroll(500.0, 4_900.0, 0), ListWindow::default());
    }

    #[test]
    fn revealing_a_mounted_row_keeps_the_window() {
        let window = at_row(1_000, 100_000);
        assert_eq!(window.revealing(1_000), window);
        assert_eq!(window.revealing(1_000 - OVERSCAN), window);
    }

    #[test]
    fn revealing_a_row_below_mounts_it_near_the_bottom() {
        let window = ListWindow::default().revealing(WINDOW_ROWS);
        let rendered = window.rendered(100_000, WINDOW_ROWS);
        assert!(rendered.contains(&WINDOW_ROWS));
        assert!(
            rendered.contains(&(WINDOW_ROWS - 1)),
            "the row it came from stays mounted"
        );
    }

    #[test]
    fn revealing_a_row_above_mounts_it_with_overscan() {
        let window = at_row(1_000, 100_000).revealing(10);
        assert_eq!(window.rendered(100_000, 10).first(), Some(&(10 - OVERSCAN)));
    }

    proptest! {
        /// Whatever the scroll offset and tab stop, a window mounts a bounded number of rows, ascending
        /// and in range, the stop among them.
        #[test]
        fn the_mount_is_bounded_and_holds_the_stop(
            total in 1usize..300_000, row in 0u32..400_000, stop_seed in any::<usize>(),
        ) {
            let stop = stop_seed % total;
            let rendered = at_row(row, total).rendered(total, stop);
            prop_assert!(rendered.len() <= WINDOW_ROWS + 1);
            prop_assert!(rendered.len() >= WINDOW_ROWS.min(total));
            prop_assert!(rendered.contains(&stop));
            prop_assert!(rendered.windows(2).all(|pair| pair[0] < pair[1]));
            prop_assert!(rendered.iter().all(|&index| index < total));
        }

        /// Roving focus reaches every row: from any window, revealing a row mounts it.
        #[test]
        fn revealing_mounts_any_row(
            total in 1usize..300_000, row in 0u32..400_000, target_seed in any::<usize>(),
        ) {
            let target = target_seed % total;
            let rendered = at_row(row, total).revealing(target).rendered(total, target);
            prop_assert!(rendered.contains(&target));
            prop_assert!(rendered.len() <= WINDOW_ROWS);
        }
    }

    #[test]
    fn stepping_down_and_up_reaches_every_row_of_a_long_list() {
        let total = 2_000;
        let mut window = ListWindow::default();
        for index in 0..total {
            window = window.revealing(index);
            assert!(
                window.rendered(total, index).contains(&index),
                "row {index} on the way down"
            );
        }
        for index in (0..total).rev() {
            window = window.revealing(index);
            assert!(
                window.rendered(total, index).contains(&index),
                "row {index} on the way up"
            );
        }
    }
}
