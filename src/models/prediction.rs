//! The TFL API Prediction Model
//!
//! https://api-portal.tfl.gov.uk/api-details#api=StopPoint&operation=StopPoint_ArrivalsByPathId
//! https://api-portal.tfl.gov.uk/api-details#api=StopPoint&operation=StopPoint_ArrivalsByPathId&definition=Tfl.Api.Presentation.Entities.Prediction
//! https://api.tfl.gov.uk/StopPoint/{id}/Arrivals
//!
//!
//! Note: A number of fields are commented out, this is because they are useful
//! to retain for debugging, but cannot be used normally as they take an
//! exorbitant amount of RAM for the embedded device. Only the fields that are
//! necessary for conveying information are retained.
//!
use defmt::Format;
use heapless::String;
use serde::Deserialize;

use crate::models::{TFL_API_FIELD_LONG_STR_SIZE, TFL_API_FIELD_STR_SIZE};

// use crate::models::TFL_API_FIELD_SHORT_STR_SIZE;

pub const ARRAY_MAX_SIZE_PREDICTION_MODEL: usize = 6;

#[derive(Deserialize, Debug, Format, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    // #[serde(rename = "$type")]
    // pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    // pub id: String<TFL_API_FIELD_STR_SIZE>,
    pub vehicle_id: String<TFL_API_FIELD_STR_SIZE>,
    // pub naptan_id: String<TFL_API_FIELD_STR_SIZE>,
    // pub station_name: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub destination_name: String<TFL_API_FIELD_LONG_STR_SIZE>,
    // pub line_id: String<TFL_API_FIELD_STR_SIZE>,
    // pub line_name: String<TFL_API_FIELD_STR_SIZE>,
    pub platform_name: String<TFL_API_FIELD_STR_SIZE>,
    // pub direction: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    // pub bearing: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    // pub destination_naptan_id: String<TFL_API_FIELD_STR_SIZE>,
    pub timestamp: String<TFL_API_FIELD_STR_SIZE>,
    pub time_to_station: u32,
    pub current_location: String<TFL_API_FIELD_LONG_STR_SIZE>,
    // pub towards: String<TFL_API_FIELD_STR_SIZE>,
    // pub expected_arrival: String<TFL_API_FIELD_STR_SIZE>,
    // pub time_to_live: String<TFL_API_FIELD_STR_SIZE>,
    // pub mode_name: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    // pub timing: PredictionTiming,
}

// #[derive(Deserialize, Debug, Format)]
// #[serde(rename_all = "camelCase")]
// pub struct PredictionTiming {
//     #[serde(rename = "$type")]
//     pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
//     pub countdown_server_adjustment: String<TFL_API_FIELD_STR_SIZE>,
//     pub source: String<TFL_API_FIELD_STR_SIZE>,
//     pub insert: String<TFL_API_FIELD_STR_SIZE>,
//     pub read: String<TFL_API_FIELD_STR_SIZE>,
//     pub sent: String<TFL_API_FIELD_STR_SIZE>,
//     pub received: String<TFL_API_FIELD_STR_SIZE>,
// }
