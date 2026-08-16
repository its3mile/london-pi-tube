use embassy_time::Timer;

use crate::{
    config::ScheduleConfig,
    tasks::ntp::{WALL_CLOCK, unix_to_london_time},
};

pub struct Schedule {
    // hour, minute, second
    active_seconds: u32,
    inactive_seconds: u32,
}

impl Schedule {
    pub const fn new(active_at: (u32, u32, u32), inactive_at: (u32, u32, u32)) -> Self {
        Self {
            active_seconds: Self::time_to_seconds(active_at),
            inactive_seconds: Self::time_to_seconds(inactive_at),
        }
    }

    pub const fn from_config(config: ScheduleConfig) -> Self {
        Self {
            active_seconds: Self::time_to_seconds(*config.active_at),
            inactive_seconds: Self::time_to_seconds(*config.inactive_at),
        }
    }

    /// Convert a hour, min, sec tuple into seconds
    /// This allows for simple comparisons of times within the same day
    const fn time_to_seconds(t: (u32, u32, u32)) -> u32 {
        (t.0 * 3600) + (t.1 * 60) + t.2
    }

    fn get_time() -> Option<u64> {
        WALL_CLOCK.lock(|cell| {
            let clock = cell.borrow();
            clock.current_unix()
        })
    }

    /// Whether the current time is inside the active window.
    /// This is a generic active/inactive schedule, not a strict day/night label.
    pub fn is_active(&self) -> bool {
        let current_time = Self::get_time();

        match current_time {
            Some(t) => {
                let current = unix_to_london_time(t);
                let current_seconds = Self::time_to_seconds(current);

                current_seconds >= self.active_seconds && current_seconds < self.inactive_seconds
            }
            // if somehow unable to get the current time, return true
            None => true,
        }
    }

    /// How many seconds until the next active window begins.
    pub fn seconds_until_active(&self) -> u32 {
        match Self::get_time() {
            Some(t) => {
                let current = unix_to_london_time(t);
                let current_seconds = Self::time_to_seconds(current);

                // Within active period - return 0
                if current_seconds >= self.active_seconds && current_seconds < self.inactive_seconds
                {
                    0u32
                // After inactive period - return seconds remaining in current day plus seconds until active of new day
                } else if current_seconds >= self.inactive_seconds {
                    self.active_seconds + 86400 - current_seconds
                // Before active period - return seconds until active period of current day
                } else {
                    self.active_seconds - current_seconds
                }
            }
            // unable to get time - return 0
            None => 0u32,
        }
    }

    /// Wait until the schedule is in the active window.
    pub async fn wait_until_active(&self) {
        let mut sleep_duration = self.seconds_until_active();
        while sleep_duration > 0 {
            sleep_duration = if sleep_duration > 3600 {
                3600
            } else {
                sleep_duration
            };
            Timer::after_secs(sleep_duration.into()).await;
            sleep_duration = self.seconds_until_active();
        }
    }
}
