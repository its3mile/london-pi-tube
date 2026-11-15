//! Device configuration: the types, their size limits and validation.
//!
//! There are two forms of the same configuration:
//!
//! - [`ConfigDefaults`], the compiled-in values written in the firmware's
//!   `config.rs`. Borrowed `&'static str`s, so it can be a `const`, and
//!   [`ConfigDefaults::check`] validates it while the firmware compiles.
//! - [`Config`], the owned, fixed-capacity form the firmware runs on. It can be
//!   built at runtime (e.g. loaded from flash, or received over an API) and is
//!   checked with [`Config::validate`].
//!
//! Both are validated by the same field rules below, so a value accepted at
//! build time is accepted at runtime and vice versa.

use core::fmt;

use heapless::String;
use serde::{Deserialize, Serialize};

use crate::models::TFL_API_FIELD_STR_SIZE;

/// 802.11 limit on SSID length.
pub const WIFI_SSID_MAX_LEN: usize = 32;
/// WPA2/WPA3 passphrase limit (8 to 63 characters).
pub const WIFI_PASSWORD_MAX_LEN: usize = 63;
pub const WIFI_PASSWORD_MIN_LEN: usize = 8;
pub const TFL_BASE_URL_MAX_LEN: usize = 64;
/// TfL app keys are 32 hex characters; leave some headroom.
pub const TFL_API_KEY_MAX_LEN: usize = 64;
/// e.g. `district`, `hammersmith-city`, `elizabeth`.
pub const TFL_LINE_ID_MAX_LEN: usize = 32;
/// NaPTAN codes, e.g. `940GZZLUEPY`.
pub const TFL_STOPCODE_MAX_LEN: usize = 16;
/// Matched against the API's `platformName`, so it can be no longer than it.
pub const TFL_PLATFORM_NAME_MAX_LEN: usize = TFL_API_FIELD_STR_SIZE;
/// TfL allows 500 requests a minute per key; a few per cycle is far below that.
pub const QUERY_INTERVAL_MIN_SECS: u16 = 10;
pub const QUERY_INTERVAL_MAX_SECS: u16 = 3600;

// ---------------------------------------------------------------------------
// Secret
// ---------------------------------------------------------------------------

/// A value that must not appear in logs.
///
/// `Debug` and `defmt::Format` print `***`; read the value with
/// [`Secret::expose`].
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    pub const fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[cfg(feature = "defmt")]
impl<T> defmt::Format for Secret<T> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "***")
    }
}

// ---------------------------------------------------------------------------
// TimeOfDay
// ---------------------------------------------------------------------------

/// A local time of day, valid by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(try_from = "(u8, u8, u8)", into = "(u8, u8, u8)")]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
    second: u8,
}

impl TimeOfDay {
    /// `None` unless `hour < 24`, `minute < 60` and `second < 60`.
    pub const fn new(hour: u8, minute: u8, second: u8) -> Option<Self> {
        if hour < 24 && minute < 60 && second < 60 {
            Some(Self { hour, minute, second })
        } else {
            None
        }
    }

    /// For constants: an out-of-range time is a compile error when used in a
    /// `const`, e.g. `TimeOfDay::hms(25, 0, 0)`.
    pub const fn hms(hour: u8, minute: u8, second: u8) -> Self {
        match Self::new(hour, minute, second) {
            Some(t) => t,
            None => panic!("time of day out of range: needs hour < 24, minute < 60, second < 60"),
        }
    }

    pub const fn as_hms(&self) -> (u32, u32, u32) {
        (self.hour as u32, self.minute as u32, self.second as u32)
    }
}

impl TryFrom<(u8, u8, u8)> for TimeOfDay {
    type Error = ConfigError;

    fn try_from((h, m, s): (u8, u8, u8)) -> Result<Self, Self::Error> {
        Self::new(h, m, s).ok_or(ConfigError::TimeOfDay)
    }
}

impl From<TimeOfDay> for (u8, u8, u8) {
    fn from(t: TimeOfDay) -> Self {
        (t.hour, t.minute, t.second)
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigError {
    WifiSsid,
    WifiPassword,
    TflBaseUrl,
    TflApiKey,
    TflLineId,
    TflStopcode,
    TflPlatformName,
    QueryInterval,
    TimeOfDay,
}

impl ConfigError {
    /// What is wrong and what is accepted. A `&'static str` so it can also be
    /// used in compile-time panics.
    pub const fn message(self) -> &'static str {
        match self {
            Self::WifiSsid => "wifi.ssid: must be 1 to 32 bytes",
            Self::WifiPassword => {
                "wifi.password: must be empty (open network) or 8 to 63 printable ASCII characters"
            }
            Self::TflBaseUrl => {
                "tfl.base_url: must start with https://, have no trailing '/', and be at most 64 bytes"
            }
            Self::TflApiKey => "tfl.api_key: must be 1 to 64 ASCII letters or digits",
            Self::TflLineId => {
                "tfl.line_id: must be 1 to 32 lowercase ASCII letters, digits or '-', e.g. \"district\""
            }
            Self::TflStopcode => {
                "tfl.stopcode: must be 1 to 16 uppercase ASCII letters or digits, e.g. \"940GZZLUEPY\""
            }
            Self::TflPlatformName => "tfl.platform_name: must be 1 to 32 bytes",
            Self::QueryInterval => "query_interval_secs: must be 10 to 3600",
            Self::TimeOfDay => "schedule: time of day out of range",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

// ---------------------------------------------------------------------------
// Field rules, shared by `ConfigDefaults` (at compile time) and `Config`
// ---------------------------------------------------------------------------

/// Whether every byte `b` of `$s` satisfies `$ok`. A macro rather than a
/// function taking a predicate, because `const fn` cannot call function pointers.
macro_rules! all_bytes {
    ($s:expr, |$b:ident| $ok:expr) => {{
        let bytes = $s.as_bytes();
        let mut all = true;
        let mut i = 0;
        while all && i < bytes.len() {
            let $b = bytes[i];
            all = $ok;
            i += 1;
        }
        all
    }};
}

const fn len_between(s: &str, min: usize, max: usize) -> bool {
    s.len() >= min && s.len() <= max
}

const fn check(ok: bool, error: ConfigError) -> Result<(), ConfigError> {
    if ok { Ok(()) } else { Err(error) }
}

pub const fn validate_wifi_ssid(s: &str) -> Result<(), ConfigError> {
    check(len_between(s, 1, WIFI_SSID_MAX_LEN), ConfigError::WifiSsid)
}

pub const fn validate_wifi_password(s: &str) -> Result<(), ConfigError> {
    let ok = s.is_empty()
        || (len_between(s, WIFI_PASSWORD_MIN_LEN, WIFI_PASSWORD_MAX_LEN)
            && all_bytes!(s, |b| b.is_ascii_graphic() || b == b' '));
    check(ok, ConfigError::WifiPassword)
}

pub const fn validate_tfl_base_url(s: &str) -> Result<(), ConfigError> {
    let bytes = s.as_bytes();
    let https = b"https://";
    let mut ok = s.len() > https.len() && s.len() <= TFL_BASE_URL_MAX_LEN;
    let mut i = 0;
    while ok && i < https.len() {
        ok = bytes[i] == https[i];
        i += 1;
    }
    ok = ok && bytes[bytes.len() - 1] != b'/' && all_bytes!(s, |b| b.is_ascii_graphic());
    check(ok, ConfigError::TflBaseUrl)
}

pub const fn validate_tfl_api_key(s: &str) -> Result<(), ConfigError> {
    // Goes into the query string unencoded, hence the restricted alphabet.
    let ok = len_between(s, 1, TFL_API_KEY_MAX_LEN) && all_bytes!(s, |b| b.is_ascii_alphanumeric());
    check(ok, ConfigError::TflApiKey)
}

pub const fn validate_tfl_line_id(s: &str) -> Result<(), ConfigError> {
    let ok = len_between(s, 1, TFL_LINE_ID_MAX_LEN)
        && all_bytes!(s, |b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    check(ok, ConfigError::TflLineId)
}

pub const fn validate_tfl_stopcode(s: &str) -> Result<(), ConfigError> {
    let ok = len_between(s, 1, TFL_STOPCODE_MAX_LEN)
        && all_bytes!(s, |b| b.is_ascii_uppercase() || b.is_ascii_digit());
    check(ok, ConfigError::TflStopcode)
}

pub const fn validate_tfl_platform_name(s: &str) -> Result<(), ConfigError> {
    check(len_between(s, 1, TFL_PLATFORM_NAME_MAX_LEN), ConfigError::TflPlatformName)
}

pub const fn validate_query_interval(secs: u16) -> Result<(), ConfigError> {
    check(
        secs >= QUERY_INTERVAL_MIN_SECS && secs <= QUERY_INTERVAL_MAX_SECS,
        ConfigError::QueryInterval,
    )
}

/// `?` for `const fn`, which cannot use the operator.
macro_rules! try_const {
    ($e:expr) => {
        if let Err(e) = $e {
            return Err(e);
        }
    };
}

// ---------------------------------------------------------------------------
// Compiled-in defaults
// ---------------------------------------------------------------------------

/// The compiled-in configuration, as written in the firmware's `config.rs`.
#[derive(Clone, Copy)]
pub struct ConfigDefaults {
    pub wifi: WifiDefaults,
    pub tfl: TflDefaults,
    pub schedule: ScheduleConfig,
    /// Seconds between request cycles.
    pub query_interval_secs: u16,
}

#[derive(Clone, Copy)]
pub struct WifiDefaults {
    pub ssid: &'static str,
    pub password: &'static str,
}

#[derive(Clone, Copy)]
pub struct TflDefaults {
    /// API root, without a trailing `/`.
    pub base_url: &'static str,
    pub api_key: &'static str,
    pub line_id: &'static str,
    pub stopcode: &'static str,
    /// Exactly as TfL reports it in `platformName`, e.g. `"Platform 1"`.
    pub platform_name: &'static str,
    /// Optional `direction=` filter on the arrivals request; see [`Direction`].
    pub direction: Option<Direction>,
}

impl ConfigDefaults {
    pub const fn validate(&self) -> Result<(), ConfigError> {
        try_const!(validate_wifi_ssid(self.wifi.ssid));
        try_const!(validate_wifi_password(self.wifi.password));
        try_const!(validate_tfl_base_url(self.tfl.base_url));
        try_const!(validate_tfl_api_key(self.tfl.api_key));
        try_const!(validate_tfl_line_id(self.tfl.line_id));
        try_const!(validate_tfl_stopcode(self.tfl.stopcode));
        try_const!(validate_tfl_platform_name(self.tfl.platform_name));
        validate_query_interval(self.query_interval_secs)
    }

    /// Validate in a `const` context, so an invalid value fails the build:
    ///
    /// ```ignore
    /// const _: () = DEFAULTS.check();
    /// ```
    pub const fn check(&self) {
        if let Err(e) = self.validate() {
            panic!("{}", e.message());
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime configuration
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Config {
    pub wifi: WifiConfig,
    pub tfl: TflConfig,
    pub schedule: ScheduleConfig,
    /// Seconds between request cycles.
    pub query_interval_secs: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WifiConfig {
    pub ssid: String<WIFI_SSID_MAX_LEN>,
    pub password: Secret<String<WIFI_PASSWORD_MAX_LEN>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TflConfig {
    pub base_url: String<TFL_BASE_URL_MAX_LEN>,
    pub api_key: Secret<String<TFL_API_KEY_MAX_LEN>>,
    pub line_id: String<TFL_LINE_ID_MAX_LEN>,
    pub stopcode: String<TFL_STOPCODE_MAX_LEN>,
    pub platform_name: String<TFL_PLATFORM_NAME_MAX_LEN>,
    pub direction: Option<Direction>,
}

/// TfL's `direction` filter for `/Line/{line}/Arrivals/{stop}`.
///
/// TfL filters on each train's `direction` field, which is sometimes blank:
/// often on the District and Elizabeth lines. A train with a blank direction
/// is dropped by `Inbound` and `Outbound`, so on those lines setting this can
/// hide trains that are coming. Leave it as `None` there; the platform filter
/// already picks the right trains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Inbound,
    Outbound,
    All,
}

impl Direction {
    /// The value TfL expects in the query string.
    pub const fn as_query(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::All => "all",
        }
    }
}

/// Daily active window: the device sleeps outside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ScheduleConfig {
    pub active_at: TimeOfDay,
    pub inactive_at: TimeOfDay,
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_wifi_ssid(&self.wifi.ssid)?;
        validate_wifi_password(self.wifi.password.expose())?;
        validate_tfl_base_url(&self.tfl.base_url)?;
        validate_tfl_api_key(self.tfl.api_key.expose())?;
        validate_tfl_line_id(&self.tfl.line_id)?;
        validate_tfl_stopcode(&self.tfl.stopcode)?;
        validate_tfl_platform_name(&self.tfl.platform_name)?;
        validate_query_interval(self.query_interval_secs)
    }
}

impl TryFrom<&ConfigDefaults> for Config {
    type Error = ConfigError;

    fn try_from(d: &ConfigDefaults) -> Result<Self, Self::Error> {
        // Validating first also guarantees every string fits its capacity.
        d.validate()?;
        Ok(Self {
            wifi: WifiConfig {
                ssid: copy(d.wifi.ssid, ConfigError::WifiSsid)?,
                password: Secret::new(copy(d.wifi.password, ConfigError::WifiPassword)?),
            },
            tfl: TflConfig {
                base_url: copy(d.tfl.base_url, ConfigError::TflBaseUrl)?,
                api_key: Secret::new(copy(d.tfl.api_key, ConfigError::TflApiKey)?),
                line_id: copy(d.tfl.line_id, ConfigError::TflLineId)?,
                stopcode: copy(d.tfl.stopcode, ConfigError::TflStopcode)?,
                platform_name: copy(d.tfl.platform_name, ConfigError::TflPlatformName)?,
                direction: d.tfl.direction,
            },
            schedule: d.schedule,
            query_interval_secs: d.query_interval_secs,
        })
    }
}

fn copy<const N: usize>(s: &str, error: ConfigError) -> Result<String<N>, ConfigError> {
    String::try_from(s).map_err(|_| error)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: ConfigDefaults = ConfigDefaults {
        wifi: WifiDefaults {
            ssid: "home",
            password: "correct horse battery",
        },
        tfl: TflDefaults {
            base_url: "https://api.tfl.gov.uk",
            api_key: "0123456789abcdef0123456789abcdef",
            line_id: "hammersmith-city",
            stopcode: "940GZZLUEPY",
            platform_name: "Platform 1",
            direction: Some(Direction::Outbound),
        },
        schedule: ScheduleConfig {
            active_at: TimeOfDay::hms(6, 30, 0),
            inactive_at: TimeOfDay::hms(22, 30, 0),
        },
        query_interval_secs: 30,
    };

    // The compile-time check accepts the valid example.
    const _: () = VALID.check();

    #[test]
    fn valid_defaults_convert() {
        let config = Config::try_from(&VALID).unwrap();
        assert_eq!(config.validate(), Ok(()));
        assert_eq!(config.wifi.ssid, "home");
        assert_eq!(config.tfl.api_key.expose(), "0123456789abcdef0123456789abcdef");
        assert_eq!(config.schedule.active_at.as_hms(), (6, 30, 0));
        assert_eq!(config.tfl.direction, Some(Direction::Outbound));
    }

    #[test]
    fn direction_query_values() {
        // The values TfL accepts; anything else is an HTTP 400.
        assert_eq!(Direction::Inbound.as_query(), "inbound");
        assert_eq!(Direction::Outbound.as_query(), "outbound");
        assert_eq!(Direction::All.as_query(), "all");
    }

    #[test]
    fn secrets_are_not_printed() {
        let config = Config::try_from(&VALID).unwrap();
        let printed = format!("{config:?}");
        assert!(!printed.contains("correct horse"), "{printed}");
        assert!(!printed.contains("0123456789abcdef"), "{printed}");
        assert!(printed.contains("***"), "{printed}");
    }

    #[test]
    fn wifi_rules() {
        assert_eq!(validate_wifi_ssid(""), Err(ConfigError::WifiSsid));
        assert_eq!(validate_wifi_ssid(&"x".repeat(32)), Ok(()));
        assert_eq!(validate_wifi_ssid(&"x".repeat(33)), Err(ConfigError::WifiSsid));

        assert_eq!(validate_wifi_password(""), Ok(()), "open network");
        assert_eq!(validate_wifi_password("1234567"), Err(ConfigError::WifiPassword));
        assert_eq!(validate_wifi_password("12345678"), Ok(()));
        assert_eq!(validate_wifi_password(r"p@ss w0rd\!"), Ok(()));
        assert_eq!(validate_wifi_password(&"x".repeat(64)), Err(ConfigError::WifiPassword));
        assert_eq!(validate_wifi_password("pass\nword"), Err(ConfigError::WifiPassword));
    }

    #[test]
    fn tfl_rules() {
        assert_eq!(validate_tfl_base_url("https://api.tfl.gov.uk"), Ok(()));
        assert_eq!(validate_tfl_base_url("https://api.tfl.gov.uk/"), Err(ConfigError::TflBaseUrl));
        assert_eq!(validate_tfl_base_url("http://api.tfl.gov.uk"), Err(ConfigError::TflBaseUrl));
        assert_eq!(validate_tfl_base_url("https://"), Err(ConfigError::TflBaseUrl));

        assert_eq!(validate_tfl_api_key(""), Err(ConfigError::TflApiKey));
        assert_eq!(validate_tfl_api_key("abc&x=1"), Err(ConfigError::TflApiKey));

        assert_eq!(validate_tfl_line_id("elizabeth"), Ok(()));
        assert_eq!(validate_tfl_line_id("District"), Err(ConfigError::TflLineId));

        assert_eq!(validate_tfl_stopcode("910GPADTON"), Ok(()));
        assert_eq!(validate_tfl_stopcode("940gzzluepy"), Err(ConfigError::TflStopcode));

        assert_eq!(validate_tfl_platform_name(""), Err(ConfigError::TflPlatformName));
        assert_eq!(
            validate_tfl_platform_name(&"x".repeat(33)),
            Err(ConfigError::TflPlatformName)
        );
    }

    #[test]
    fn query_interval_rules() {
        assert_eq!(validate_query_interval(9), Err(ConfigError::QueryInterval));
        assert_eq!(validate_query_interval(10), Ok(()));
        assert_eq!(validate_query_interval(3600), Ok(()));
        assert_eq!(validate_query_interval(3601), Err(ConfigError::QueryInterval));
    }

    #[test]
    fn invalid_defaults_are_rejected() {
        let mut d = VALID;
        d.tfl.stopcode = "not a stopcode";
        assert_eq!(Config::try_from(&d), Err(ConfigError::TflStopcode));
    }

    #[test]
    fn time_of_day() {
        assert!(TimeOfDay::new(23, 59, 59).is_some());
        assert!(TimeOfDay::new(24, 0, 0).is_none());
        assert!(TimeOfDay::new(0, 60, 0).is_none());
        assert_eq!(TimeOfDay::try_from((25, 0, 0)), Err(ConfigError::TimeOfDay));
    }

    #[test]
    fn longest_urls_fit_the_request_buffer() {
        // request.rs builds URLs in a String<256>.
        let longest = TFL_BASE_URL_MAX_LEN
            + "/Line/".len()
            + TFL_LINE_ID_MAX_LEN
            + "/Arrivals/".len()
            + TFL_STOPCODE_MAX_LEN
            + "?direction=".len()
            + Direction::Outbound.as_query().len()
            + "&app_key=".len()
            + TFL_API_KEY_MAX_LEN;
        assert!(longest <= 256, "{longest}");
    }
}
