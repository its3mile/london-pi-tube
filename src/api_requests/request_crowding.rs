use ::function_name::named;
use const_format::formatcp;
use defmt::{error, info};
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::TcpClient;
use reqwless::client::HttpClient;
use reqwless::request::Method;

use crate::api_requests::models::crowding::Crowding;
use crate::api_requests::{HTTP_PROXY, TFL_API_PRIMARY_KEY, TFL_STOPCODE_PARAM};

// define the URL for the TFL API request
const CROWDING_URL: &str = formatcp!("{HTTP_PROXY}/Crowding/{TFL_STOPCODE_PARAM}/Live?api_key={TFL_API_PRIMARY_KEY}");

#[named]
pub async fn request_crowding(
    http_client: &mut HttpClient<'_, TcpClient<'_, 1>, DnsSocket<'_>>,
    rx_buffer: &mut [u8],
) -> Option<Crowding> {
    // Make the HTTP request to the TFL API
    info!("{}: connecting to {}", function_name!(), &CROWDING_URL);

    // 1. Make HTTP request
    let mut request = match http_client.request(Method::GET, &CROWDING_URL).await {
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
    match serde_json_core::de::from_slice::<Crowding>(&body) {
        Ok((crowding, used)) => {
            info!("{}: Used {} bytes from the response body", function_name!(), used);
            Some(crowding)
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
