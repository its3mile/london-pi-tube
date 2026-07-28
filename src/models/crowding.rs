use defmt::Format;
use serde::Deserialize;

#[derive(Deserialize, Debug, Format)]
#[serde(rename_all = "camelCase")]
pub struct Crowding {
    pub data_available: bool,
    pub percentage_of_baseline: f64,
    // Incomplete implementation, as much of the data is not required
}
