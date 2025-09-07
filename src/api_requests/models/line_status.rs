use defmt::Format;
use heapless::String;
use serde::Deserialize;

use crate::api_requests::models::{TFL_API_FIELD_LONG_STR_SIZE, TFL_API_FIELD_SHORT_STR_SIZE};

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct LineStatus {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub status_severity_description: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    // Incomplete implementation, as much of the data is not required
}
