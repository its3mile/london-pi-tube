//! Display update model
//!
//! This collates the data from various requests and configurations.
//!
use defmt::Format;
use heapless::{String, Vec};

use crate::models::prediction::{ARRAY_MAX_SIZE_PREDICTION_MODEL, Prediction};
use crate::models::{
    TFL_API_FIELD_LONG_STR_SIZE, TFL_API_FIELD_SHORT_STR_SIZE, TFL_API_FIELD_STR_SIZE,
    WORLD_TIME_API_FIELD_STR_SIZE,
};

#[derive(Debug, Format, Clone)]
pub struct Update {
    pub datetime: String<WORLD_TIME_API_FIELD_STR_SIZE>,
    pub arrivals: Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>,
    pub line_name: String<TFL_API_FIELD_STR_SIZE>,
    pub line_status: String<TFL_API_FIELD_SHORT_STR_SIZE>,
    pub platform_name: String<TFL_API_FIELD_STR_SIZE>,
    pub station_name: String<TFL_API_FIELD_LONG_STR_SIZE>,
}

impl Update {
    pub const fn new() -> Self {
        Self {
            datetime: String::<WORLD_TIME_API_FIELD_STR_SIZE>::new(),
            arrivals: Vec::new(),
            line_name: String::<TFL_API_FIELD_STR_SIZE>::new(),
            line_status: String::<TFL_API_FIELD_SHORT_STR_SIZE>::new(),
            platform_name: String::<TFL_API_FIELD_STR_SIZE>::new(),
            station_name: String::<TFL_API_FIELD_LONG_STR_SIZE>::new(),
        }
    }
}
