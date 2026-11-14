//! Daily active/inactive window maths.
//!
//! Times are seconds since local midnight. The firmware's `Schedule` supplies
//! the current time from the NTP-synced wall clock.

pub const SECS_PER_DAY: u32 = 86400;

/// A daily window during which the device is active.
///
/// The window may wrap past midnight, e.g. active 22:00, inactive 06:00.
/// If `active_at == inactive_at`, the window is empty (never active).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveWindow {
    active_seconds: u32,
    inactive_seconds: u32,
}

impl ActiveWindow {
    /// `active_at` and `inactive_at` are local (hour, minute, second).
    pub const fn new(active_at: (u32, u32, u32), inactive_at: (u32, u32, u32)) -> Self {
        Self {
            active_seconds: hms_to_seconds(active_at),
            inactive_seconds: hms_to_seconds(inactive_at),
        }
    }

    pub const fn is_active_at(&self, seconds_of_day: u32) -> bool {
        let (a, i, t) = (self.active_seconds, self.inactive_seconds, seconds_of_day);
        if a <= i {
            a <= t && t < i
        } else {
            t >= a || t < i
        }
    }

    /// Seconds until the window next opens; 0 if currently active.
    pub const fn seconds_until_active_at(&self, seconds_of_day: u32) -> u32 {
        if self.is_active_at(seconds_of_day) {
            return 0;
        }
        let until = (self.active_seconds + SECS_PER_DAY - seconds_of_day) % SECS_PER_DAY;
        // Only reachable for an empty window at exactly `active_at`: wait a
        // full day rather than return 0 and make callers spin.
        if until == 0 { SECS_PER_DAY } else { until }
    }
}

pub const fn hms_to_seconds(t: (u32, u32, u32)) -> u32 {
    t.0 * 3600 + t.1 * 60 + t.2
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn s(h: u32, m: u32) -> u32 {
        hms_to_seconds((h, m, 0))
    }

    #[test]
    fn same_day_window() {
        let w = ActiveWindow::new((6, 30, 0), (22, 30, 0));
        assert!(!w.is_active_at(s(6, 29)));
        assert!(w.is_active_at(s(6, 30)));
        assert!(w.is_active_at(s(22, 29)));
        assert!(!w.is_active_at(s(22, 30)));

        assert_eq!(w.seconds_until_active_at(s(12, 0)), 0);
        assert_eq!(w.seconds_until_active_at(s(6, 0)), 30 * 60);
        assert_eq!(w.seconds_until_active_at(s(23, 0)), s(7, 30));
        assert_eq!(w.seconds_until_active_at(0), s(6, 30));
    }

    #[test]
    fn overnight_window() {
        let w = ActiveWindow::new((22, 0, 0), (6, 0, 0));
        assert!(w.is_active_at(s(23, 0)));
        assert!(w.is_active_at(s(2, 0)));
        assert!(!w.is_active_at(s(6, 0)));
        assert!(!w.is_active_at(s(12, 0)));

        assert_eq!(w.seconds_until_active_at(s(2, 0)), 0);
        assert_eq!(w.seconds_until_active_at(s(12, 0)), s(10, 0));
    }

    #[test]
    fn empty_window_never_spins() {
        let w = ActiveWindow::new((8, 0, 0), (8, 0, 0));
        for t in [0, s(7, 59), s(8, 0), s(8, 1), s(23, 59)] {
            assert!(!w.is_active_at(t));
            assert!(w.seconds_until_active_at(t) > 0);
        }
    }
}
