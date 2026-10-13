//! Small presentation widgets shared by native and ANSI screens.
use std::time::Duration;

pub const REFRESH_SEGMENTS: usize = 8;

/// A strip clearing left to right when waiting; one segment moving left to right while refreshing.
/// This is presentation only. The caller owns the request and refresh deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshIndicator {
    pub mask: u8,
}

impl RefreshIndicator {
    pub fn waiting(remaining: Duration, interval: Duration) -> Self {
        let fraction = if interval.is_zero() {
            0.0
        } else {
            (remaining.as_secs_f64() / interval.as_secs_f64()).clamp(0.0, 1.0)
        };
        let count = (fraction * REFRESH_SEGMENTS as f64).ceil() as usize;
        Self {
            mask: (((1u16 << count) - 1) << (REFRESH_SEGMENTS - count)) as u8,
        }
    }

    pub fn refreshing(elapsed: Duration) -> Self {
        Self {
            mask: 1 << ((elapsed.as_millis() / 150) % REFRESH_SEGMENTS as u128),
        }
    }

    pub fn is_lit(self, segment: usize) -> bool {
        segment < REFRESH_SEGMENTS && self.mask & (1 << segment) != 0
    }

    /// Position inside the viewport, one cell inset from the bottom and right.
    /// Save/restore cursor and reset colours so this can overlay any ANSI screen.
    pub fn ansi_bottom_right(self, columns: usize, rows: usize) -> String {
        if columns < REFRESH_SEGMENTS + 2 || rows < 2 {
            return String::new();
        }
        let mut output = format!("\x1b7\x1b[{};{}H", rows - 1, columns - REFRESH_SEGMENTS);
        for segment in 0..REFRESH_SEGMENTS {
            output.push_str(if self.is_lit(segment) {
                "\x1b[97m#"
            } else {
                "\x1b[90m-"
            });
        }
        output.push_str("\x1b[0m\x1b8");
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_interval_lights_all_segments() {
        assert_eq!(
            RefreshIndicator::waiting(Duration::from_secs(60), Duration::from_secs(60)).mask,
            255
        );
    }
    #[test]
    fn halfway_lights_four_segments() {
        assert_eq!(
            RefreshIndicator::waiting(Duration::from_secs(30), Duration::from_secs(60)).mask,
            240
        );
    }
    #[test]
    fn expired_interval_is_empty() {
        assert_eq!(
            RefreshIndicator::waiting(Duration::ZERO, Duration::from_secs(60)).mask,
            0
        );
    }
    #[test]
    fn zero_interval_does_not_divide_by_zero() {
        assert_eq!(
            RefreshIndicator::waiting(Duration::from_secs(1), Duration::ZERO).mask,
            0
        );
    }
    #[test]
    fn remaining_is_clamped_to_full() {
        assert_eq!(
            RefreshIndicator::waiting(Duration::from_secs(120), Duration::from_secs(60)).mask,
            255
        );
    }
    #[test]
    fn busy_segment_moves_and_wraps() {
        assert_eq!(
            RefreshIndicator::refreshing(Duration::from_millis(1200)),
            RefreshIndicator::refreshing(Duration::ZERO)
        );
    }
    #[test]
    fn waiting_clears_leftmost_segment_first() {
        let indicator = RefreshIndicator::waiting(Duration::from_secs(52), Duration::from_secs(60));
        assert!(!indicator.is_lit(0));
        assert!(indicator.is_lit(7));
    }
    #[test]
    fn busy_segment_moves_from_left_to_right() {
        assert_eq!(RefreshIndicator::refreshing(Duration::ZERO).mask, 1);
        assert_eq!(
            RefreshIndicator::refreshing(Duration::from_millis(150)).mask,
            2
        );
        assert_eq!(
            RefreshIndicator::refreshing(Duration::from_millis(1050)).mask,
            128
        );
    }
    #[test]
    fn busy_has_exactly_one_lit_segment() {
        assert_eq!(
            RefreshIndicator::refreshing(Duration::from_millis(450))
                .mask
                .count_ones(),
            1
        );
    }
    #[test]
    fn tiny_grid_omits_widget() {
        assert!(
            RefreshIndicator { mask: 255 }
                .ansi_bottom_right(8, 1)
                .is_empty()
        );
    }
    #[test]
    fn overlay_is_inset_in_bottom_right() {
        assert!(
            RefreshIndicator { mask: 255 }
                .ansi_bottom_right(60, 40)
                .contains("\x1b[39;52H")
        );
    }
}
