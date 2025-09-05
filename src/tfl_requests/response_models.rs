use defmt::Format;
use heapless::String;
use heapless::Vec;
use serde::Deserialize;

pub const TFL_API_FIELD_SHORT_STR_SIZE: usize = 32;
pub const TFL_API_FIELD_STR_SIZE: usize = 64;
pub const TFL_API_FIELD_LONG_STR_SIZE: usize = 128;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct PredictionTiming {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub countdown_server_adjustment: String<TFL_API_FIELD_STR_SIZE>,
    pub source: String<TFL_API_FIELD_STR_SIZE>,
    pub insert: String<TFL_API_FIELD_STR_SIZE>,
    pub read: String<TFL_API_FIELD_STR_SIZE>,
    pub sent: String<TFL_API_FIELD_STR_SIZE>,
    pub received: String<TFL_API_FIELD_STR_SIZE>,
}

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

pub const ARRAY_MAX_SIZE_PREDICTION_MODEL: usize = 8;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub line_statuses: Vec<LineStatus, 1>,
    // Incomplete implementation, as much of the data is not required
}

pub const ARRAY_MAX_SIZE_STATUS_MODEL: usize = 1;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct LineStatus {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub status_severity_description: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    // Incomplete implementation, as much of the data is not required
}

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct Crowding {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub data_available: bool,
    pub percentage_of_baseline: f64,
    // Incomplete implementation, as much of the data is not required
}
