// IMPORTANT: Copy this file to config.rs (in the src directory) and update with your real credentials
// DO NOT commit config.rs to Git - it should be (already) in .gitignore
//
// These are the compiled-in defaults the device boots with. They are checked
// while the firmware compiles: an invalid value (e.g. a stop code in lowercase,
// or a time of 25:00) fails the build with a message naming the field. The
// rules are in application/src/config.rs.
// `Direction` is only needed once `direction` is set.
#[allow(unused_imports)]
use application::config::{
    ConfigDefaults, Direction, ScheduleConfig, TflDefaults, TimeOfDay, WifiDefaults,
};

pub const DEFAULTS: ConfigDefaults = ConfigDefaults {
    wifi: WifiDefaults {
        ssid: "your-ssid",
        // Raw string, so backslashes and quotes need no escaping.
        password: r"your-wifi-password",
    },
    tfl: TflDefaults {
        base_url: "https://api.tfl.gov.uk",
        api_key: "your-tfl-api-key",
        line_id: "district",
        stopcode: "940GZZLUEPY",
        // Exactly as TfL reports it in `platformName`.
        platform_name: "Platform 1",
        // Optional server-side filter: Some(Direction::Inbound), Some(Direction::Outbound)
        // or Some(Direction::All). TfL sometimes leaves a train's direction blank
        // (often on the District and Elizabeth lines) and the filter then drops
        // it, so leave this as None on those lines.
        direction: None,
    },
    // The device sleeps outside this daily window (London time).
    schedule: ScheduleConfig {
        active_at: TimeOfDay::hms(6, 30, 0),
        inactive_at: TimeOfDay::hms(22, 30, 0),
    },
    // Seconds between request cycles.
    query_interval_secs: 30,
};
