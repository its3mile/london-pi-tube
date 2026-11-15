//! TfL API models.
//!
//! Only the fields needed for display are kept: each retained `String<N>`
//! costs N bytes of RAM per element. Every other field in the response is
//! skipped by the parser.
//!
//! String fields use [`truncating_string`] and `#[serde(default)]`, so a field
//! that is longer than expected, `null`, or missing degrades that one value
//! instead of failing the entire response.
//!
//! References:
//! - Prediction: <https://api-portal.tfl.gov.uk/api-details#api=StopPoint&operation=StopPoint_ArrivalsByPathId>
//! - Line status: <https://api-portal.tfl.gov.uk/api-details#api=Line&operation=Line_StatusByIdsByPathIdsQueryDetail>

use heapless::String;
use serde::Deserialize;

use crate::de::{first_element, truncating_string};

pub const TFL_API_FIELD_SHORT_STR_SIZE: usize = 16;
pub const TFL_API_FIELD_STR_SIZE: usize = 32;
pub const TFL_API_FIELD_LONG_STR_SIZE: usize = 72;

/// Maximum number of predictions kept *after* filtering by platform.
///
/// The response itself may contain any number of predictions; see
/// [`crate::tfl::parse_predictions`].
pub const ARRAY_MAX_SIZE_PREDICTION_MODEL: usize = 8;

/// One element of `GET /StopPoint/{id}/Arrivals`.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    #[serde(default, deserialize_with = "truncating_string")]
    pub vehicle_id: String<TFL_API_FIELD_STR_SIZE>,
    #[serde(default, deserialize_with = "truncating_string")]
    pub station_name: String<TFL_API_FIELD_LONG_STR_SIZE>,
    /// Frequently absent from real responses.
    #[serde(
        default = "default_destination_name",
        deserialize_with = "truncating_string"
    )]
    pub destination_name: String<TFL_API_FIELD_LONG_STR_SIZE>,
    #[serde(default, deserialize_with = "truncating_string")]
    pub line_name: String<TFL_API_FIELD_STR_SIZE>,
    #[serde(default, deserialize_with = "truncating_string")]
    pub platform_name: String<TFL_API_FIELD_STR_SIZE>,
    #[serde(default, deserialize_with = "truncating_string")]
    pub timestamp: String<TFL_API_FIELD_STR_SIZE>,
    /// Seconds until arrival. Required: a prediction without it is useless.
    pub time_to_station: u32,
    #[serde(default, deserialize_with = "truncating_string")]
    pub current_location: String<TFL_API_FIELD_LONG_STR_SIZE>,
}

fn default_destination_name() -> String<TFL_API_FIELD_LONG_STR_SIZE> {
    crate::de::truncate("N/A")
}

/// One element of `Line.lineStatuses`.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "camelCase")]
pub struct LineStatus {
    /// e.g. "Good Service", "Part Closure", "No Step Free Access".
    #[serde(default, deserialize_with = "truncating_string")]
    pub status_severity_description: String<TFL_API_FIELD_STR_SIZE>,
}

/// One element of `GET /Line/{ids}/Status`.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Display name, e.g. "District" or "Elizabeth line".
    #[serde(default, deserialize_with = "truncating_string")]
    pub name: String<TFL_API_FIELD_STR_SIZE>,
    /// The first entry of `lineStatuses`, if any. Further entries are skipped.
    #[serde(rename = "lineStatuses", default, deserialize_with = "first_element")]
    pub line_status: Option<LineStatus>,
}
