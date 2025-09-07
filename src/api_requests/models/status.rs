use crate::api_requests::models::line_status::LineStatus;
use defmt::Format;
use heapless::String;
use heapless::Vec;
use serde::Deserialize;

use crate::api_requests::models::TFL_API_FIELD_LONG_STR_SIZE;

pub const ARRAY_MAX_SIZE_STATUS_MODEL: usize = 1;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(rename = "$type")]
    pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub line_statuses: Vec<LineStatus, 1>,
    // Incomplete implementation, as much of the data is not required
}
