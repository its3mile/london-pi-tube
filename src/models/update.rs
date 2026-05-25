//! Display update model
//!
//! This collates the data from various requests and configurations.
//!
use defmt::Format;
use heapless::Vec;

use crate::models::prediction::{ARRAY_MAX_SIZE_PREDICTION_MODEL, Prediction};

#[derive(Debug, Format, Clone)]
pub struct Update {
    pub arrivals: Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>,
    pub last_updated_secs: u32,
}
