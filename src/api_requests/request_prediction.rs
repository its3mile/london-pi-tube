use ::function_name::named;
use const_format::formatcp;
use defmt::{error, info};
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::TcpClient;
use heapless::Vec;
use reqwless::client::HttpClient;
use reqwless::request::Method;

use crate::api_requests::models::prediction::{Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL};
use crate::api_requests::{HTTP_PROXY, TFL_API_PRIMARY_KEY, TFL_PLATFORM_NAME_PARAM, TFL_STOPCODE_PARAM};

// define the URL for the TFL API request
const PREDICTION_URL: &str =
    formatcp!("{HTTP_PROXY}/StopPoint/{TFL_STOPCODE_PARAM}/Arrivals?api_key={TFL_API_PRIMARY_KEY}");

#[named]
pub async fn request_prediction(
    http_client: &mut HttpClient<'_, TcpClient<'_, 1>, DnsSocket<'_>>,
    rx_buffer: &mut [u8],
) -> Option<Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>> {
    // Make the HTTP request to the TFL API
    info!("{}: connecting to {}", function_name!(), &PREDICTION_URL);

    // 1. Make HTTP request
    let mut request = match http_client.request(Method::GET, &PREDICTION_URL).await {
        Ok(req) => req,
        Err(e) => {
            error!("{}: Failed to make HTTP request: {}", function_name!(), e);
            None?
        }
    };

    // 2. Send HTTP request
    let response = match request.send(rx_buffer).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("{}: Failed to send HTTP request: {}", function_name!(), e);
            None?
        }
    };

    // 3. Read response body
    let body = match response.body().read_to_end().await {
        Ok(body) => body,
        Err(_) => {
            error!("{}: Failed to read response body", function_name!());
            None?
        }
    };

    // 4. Process JSON objects in body
    match serde_json_core::de::from_slice::<Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>>(&body) {
        Ok((mut predictions, used)) => {
            info!("{}: Used {} bytes from the response body", function_name!(), used);

            // Retain only predictions for the platform of interest
            predictions.retain(|p| p.platform_name.contains(TFL_PLATFORM_NAME_PARAM));

            // Limit number of predictions to channel capacity
            predictions.truncate(3); // todo magic number

            // Check if there are any predictions for the platform of interest
            if predictions.is_empty() {
                info!(
                    "{}: No predictions found for platform {}",
                    function_name!(),
                    TFL_PLATFORM_NAME_PARAM
                );
                None?
            } else {
                predictions.sort_unstable_by_key(|p| p.time_to_station);
                Some(predictions)
            }
        }
        Err(e) => {
            error!("{}: Failed to deserialise JSON: {}", function_name!(), e);
            error!(
                "{}: JSON: {}",
                function_name!(),
                str::from_utf8(body).unwrap_or("Invalid UTF-8")
            );
            None?
        }
    }
}
