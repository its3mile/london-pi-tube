mod request_prediction;
mod request_status;

pub mod models;
pub mod request_task;

const TFL_API_PRIMARY_KEY: &'static str = env!("TFL_API_PRIMARY_KEY"); // define the TFL API primary key
const TFL_PLATFORM_NAME_PARAM: &'static str = env!("TFL_PLATFORM_NAME_PARAM"); // define the platform name of interest
const TFL_LINE_ID_PARAM: &'static str = env!("TFL_LINE_ID_PARAM"); // define the URL for the TFL API request
const TFL_STOPCODE_PARAM: &'static str = env!("TFL_STOPCODE_PARAM"); // define the stop code of interest

const HTTP_PROXY: &'static str = env!("HTTP_PROXY");
