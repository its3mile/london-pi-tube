use defmt::Format;
use heapless::String;
use serde::Deserialize;

use crate::api_requests::models::prediction_timing::PredictionTiming;
use crate::api_requests::models::{TFL_API_FIELD_LONG_STR_SIZE, TFL_API_FIELD_SHORT_STR_SIZE, TFL_API_FIELD_STR_SIZE};

pub const ARRAY_MAX_SIZE_PREDICTION_MODEL: usize = 8;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub id: String<TFL_API_FIELD_STR_SIZE>,
    pub operation_type: u8,
    pub vehicle_id: String<TFL_API_FIELD_STR_SIZE>,
    pub naptan_id: String<TFL_API_FIELD_STR_SIZE>,
    pub station_name: String<TFL_API_FIELD_STR_SIZE>,
    pub line_id: String<TFL_API_FIELD_STR_SIZE>,
    pub line_name: String<TFL_API_FIELD_STR_SIZE>,
    pub platform_name: String<TFL_API_FIELD_STR_SIZE>,
    pub direction: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    pub bearing: String<TFL_API_FIELD_STR_SIZE>,
    pub destination_naptan_id: String<TFL_API_FIELD_STR_SIZE>,
    pub destination_name: String<TFL_API_FIELD_STR_SIZE>,
    pub timestamp: String<TFL_API_FIELD_STR_SIZE>,
    pub time_to_station: u32,
    pub current_location: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub towards: String<TFL_API_FIELD_STR_SIZE>,
    pub expected_arrival: String<TFL_API_FIELD_STR_SIZE>,
    pub time_to_live: String<TFL_API_FIELD_STR_SIZE>,
    pub mode_name: String<TFL_API_FIELD_STR_SIZE>,
    pub timing: PredictionTiming,
}
