//! The TFL API Status Model
//!
//! https://api-portal.tfl.gov.uk/api-details#api=Line&operation=Line_StatusByIdsByPathIdsQueryDetail
//! https://api-portal.tfl.gov.uk/api-details#api=Line&operation=Line_StatusByIdsByPathIdsQueryDetail&definition=Tfl-16
//! https://api.tfl.gov.uk/Line/{ids}/Status
//!
//!
//! Note: A number of fields are commented out, this is because they are useful
//! to retain for debugging, but cannot be used normally as they take an
//! exorbitant amount of RAM for the embedded device. Only the fields that are
//! necessary for conveying information are retained.
//!
use defmt::Format;
use heapless::{String, Vec};
use serde::Deserialize;

use crate::models::TFL_API_FIELD_SHORT_STR_SIZE;

pub const ARRAY_MAX_SIZE_LINE_STATUS_MODEL: usize = 1;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct LineStatus {
    // #[serde(rename = "$type")]
    // pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub status_severity_description: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    // Incomplete implementation, as much of the data is not required
}

pub const ARRAY_MAX_SIZE_STATUS_MODEL: usize = 4;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    // #[serde(rename = "$type")]
    // pub _type: String<TFL_API_FIELD_LONG_STR_SIZE>,
    pub line_statuses: Vec<LineStatus, ARRAY_MAX_SIZE_STATUS_MODEL>,
    // Incomplete implementation, as much of the data is not required
}
