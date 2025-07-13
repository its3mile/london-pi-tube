use ::function_name::named;
use const_format::formatcp;
use core::str;
use defmt::{error, info};
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_net::Stack;
use embassy_rp::clocks::RoscRng;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::Timer;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};
use reqwless::request::Method;
use serde_json_core::de::from_slice;

use crate::string_utilities::extract_first_json_object;
use crate::tfl_requests::response_models::Prediction;
use crate::tfl_requests::{HTTP_PROXY, TFL_API_PRIMARY_KEY};

// define the URL for the TFL API request
const TFL_STOPCODE_PARAM: &'static str = env!("TFL_STOPCODE_PARAM");
const PREDICTION_URL: &str =
    formatcp!("{HTTP_PROXY}/StopPoint/{TFL_STOPCODE_PARAM}/Arrivals?api_key={TFL_API_PRIMARY_KEY}");

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn get_prediction_task(
    stack: Stack<'static>,
    tfl_api_prediction_channel_sender: Sender<'static, ThreadModeRawMutex, Prediction, 1>,
) {
    let mut rng: RoscRng = RoscRng;
    loop {
        // Create the HTTP client and DNS client
        info!("{}: Creating HTTP client and DNS client", function_name!());
        let mut rx_buffer: [u8; 8192] = [0u8; 8192];
        let mut tls_read_buffer = [0; 16640];
        let mut tls_write_buffer = [0; 16640];
        let client_state = TcpClientState::<1, 1024, 1024>::new();
        let tcp_client = TcpClient::new(stack, &client_state);
        let dns_client = DnsSocket::new(stack);
        let seed = rng.next_u64();
        let tls_config = TlsConfig::new(seed, &mut tls_read_buffer, &mut tls_write_buffer, TlsVerify::None);

        let mut http_client = HttpClient::new_with_tls(&tcp_client, &dns_client, tls_config);

        // Make the HTTP request to the TFL API
        info!("{}: connecting to {}", function_name!(), &PREDICTION_URL);

        // 1. Make HTTP request
        let mut request = match http_client.request(Method::GET, &PREDICTION_URL).await {
            Ok(req) => req,
            Err(e) => {
                error!("{}: Failed to make HTTP request: {}", function_name!(), e);
                continue;
            }
        };

        // 2. Send HTTP request
        let response = match request.send(&mut rx_buffer).await {
            Ok(resp) => resp,
            Err(e) => {
                error!("{}: Failed to send HTTP request: {}", function_name!(), e);
                continue;
            }
        };

        // 3. Read response body
        let mut body = match response.body().read_to_end().await {
            Ok(body) => body,
            Err(_) => {
                error!("{}: Failed to read response body", function_name!());
                continue;
            }
        };

        // 4. Process JSON objects in body
        let mut searching = true;
        while searching {
            if let Some(json_object) = extract_first_json_object(&body) {
                match from_slice::<Prediction>(&json_object) {
                    Ok((prediction, used)) => {
                        if prediction.platform_name.contains("Platform 1") {
                            info!("{}: Used {} bytes from the response body", function_name!(), used);
                            searching = false;
                            info!("{}: Sending preduction to display task data channel", function_name!());
                            tfl_api_prediction_channel_sender.send(prediction).await;
                            info!("{}: Sent body to display task data channel", function_name!());
                        } else {
                            body = &mut body[used..];
                        }
                    }
                    Err(e) => {
                        error!("{}: Failed to deserialise JSON: {}", function_name!(), e);
                        error!(
                            "{}: JSON: {}",
                            function_name!(),
                            str::from_utf8(json_object).unwrap_or("Invalid UTF-8")
                        );
                        continue;
                    }
                }
            } else {
                error!("{}: Could not extract JSON object from body", function_name!());
                error!(
                    "{}: UTF8: {}",
                    function_name!(),
                    str::from_utf8(body).unwrap_or("Invalid UTF-8")
                );
                continue;
            }
        }

        // Sleep for a while before the starting requests
        let query_delay_secs: u64 = option_env!("QUERY_DELAY").and_then(|s| s.parse().ok()).unwrap_or(30);
        info!(
            "{}: Waiting for {} seconds before making the request...",
            function_name!(),
            query_delay_secs
        );
        Timer::after_secs(query_delay_secs).await;
    }
}
