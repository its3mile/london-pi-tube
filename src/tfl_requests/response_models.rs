use defmt::Format;
use heapless::String;
use serde::Deserialize;

pub const TFL_API_FIELD_SHORT_STR_SIZE: usize = 32;
pub const TFL_API_FIELD_STR_SIZE: usize = 64;
pub const TFL_API_FIELD_LONG_STR_SIZE: usize = 128;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct TflApiPredictionTiming {
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
pub struct TflApiPreciction {
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
    pub timing: TflApiPredictionTiming,
}
