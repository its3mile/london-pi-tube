pub mod crowding;
pub mod prediction;
pub mod response_models;
pub mod status;

pub const TFL_API_PRIMARY_KEY: &'static str = env!("TFL_API_PRIMARY_KEY");
pub const HTTP_PROXY: &'static str = env!("HTTP_PROXY");
