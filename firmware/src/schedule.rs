//! Active/inactive schedule, driven by the NTP-synced wall clock.
//!
//! The window maths lives in `application::schedule` (host-tested); this wrapper
//! only supplies the current time and the async wait.
use application::schedule::{ActiveWindow, hms_to_seconds};
use embassy_time::Timer;

use crate::{
    config::ScheduleConfig,
    tasks::ntp::{WALL_CLOCK, unix_to_london_time},
};

pub struct Schedule {
    window: ActiveWindow,
}

impl Schedule {
    pub const fn from_config(config: ScheduleConfig) -> Self {
        Self {
            window: ActiveWindow::new(*config.active_at, *config.inactive_at),
        }
    }

    /// Seconds since local midnight, or `None` before the first NTP sync.
    fn london_seconds_of_day() -> Option<u32> {
        let unix = WALL_CLOCK.lock(|cell| cell.borrow().current_unix())?;
        Some(hms_to_seconds(unix_to_london_time(unix)))
    }

    /// Whether the current time is inside the active window.
    /// Before the clock is synced, the device is treated as active.
    pub fn is_active(&self) -> bool {
        Self::london_seconds_of_day().is_none_or(|t| self.window.is_active_at(t))
    }

    /// How many seconds until the next active window begins (0 if active or unsynced).
    pub fn seconds_until_active(&self) -> u32 {
        Self::london_seconds_of_day().map_or(0, |t| self.window.seconds_until_active_at(t))
    }

    /// Wait until the schedule is in the active window.
    pub async fn wait_until_active(&self) {
        let mut sleep_duration = self.seconds_until_active();
        while sleep_duration > 0 {
            // Re-check at least hourly, so NTP corrections are picked up.
            Timer::after_secs(sleep_duration.min(3600).into()).await;
            sleep_duration = self.seconds_until_active();
        }
    }
}
