//! World Time API Timezone Model
//!
//! via rapidapi
//!
//! https://rapidapi.com/sleeyax/api/world-time-api3/playground/apiendpoint_db563cdf-e7e0-4b20-8ea3-a2503ea0d786
//! https://timeapi.world/schema
//!
use defmt::Format;
use heapless::String;
use serde::Deserialize;

use crate::models::WORLD_TIME_API_FIELD_STR_SIZE;

#[derive(Deserialize, Debug, Format, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Timezone {
    pub datetime: String<WORLD_TIME_API_FIELD_STR_SIZE>,
    // Incomplete implementation, as much of the data is not required
}
