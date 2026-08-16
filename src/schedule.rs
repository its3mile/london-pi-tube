use crate::{
    config::ScheduleConfig,
    tasks::ntp::{WALL_CLOCK, unix_to_london_time},
};

pub struct Schedule {
    // hour, minute, second
    wake_time: (u32, u32, u32),
    sleep_time: (u32, u32, u32),
}

impl Schedule {
    pub const fn new(wake_time: (u32, u32, u32), sleep_time: (u32, u32, u32)) -> Self {
        Self {
            wake_time: wake_time,
            sleep_time: sleep_time,
        }
    }

    pub const fn from_config(config: ScheduleConfig) -> Self {
        Self {
            wake_time: *config.awake_at,
            sleep_time: *config.sleep_at,
        }
    }

    fn time_to_seconds(t: (u32, u32, u32)) -> u32 {
        (t.0 * 3600) + (t.1 * 60) + t.2
    }

    pub fn should_sleep(&self) -> bool {
        let current_time = WALL_CLOCK.lock(|cell| {
            let clock = cell.borrow();
            clock.current_unix()
        });

        match current_time {
            Some(t) => {
                let current = unix_to_london_time(t);
                let current_seconds = Self::time_to_seconds(current);
                let wake_seconds = Self::time_to_seconds(self.wake_time);
                let sleep_seconds = Self::time_to_seconds(self.sleep_time);

                !(current_seconds >= wake_seconds && current_seconds < sleep_seconds)
            }
            // if somehow unable to get the current time, return false (don't sleep)
            None => false,
        }
    }
}
