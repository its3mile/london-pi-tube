use defmt::Format;
use heapless::String;
use serde::Deserialize;

use crate::api_requests::models::{TFL_API_FIELD_LONG_STR_SIZE, TFL_API_FIELD_STR_SIZE};

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
