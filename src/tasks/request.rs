//! Request task and helper functions
//!
//! This task is responsible for performing HTTP get requests to the TFL API
//! to determine incoming trains to the configured station, platform, line
//! combination.
//!
//! Note: Due to the large memory requirements of TLS termination with an
//! external server, static buffers are used for the TLS client. This means
//! that only a single request can be performed at a time, and must be
//! processed to completion before the next.
//!
//! Buffer sizes are carefully selected to support the Pimoroni Pico Plus 2W.
//!  
use ::function_name::named;
use core::fmt::Write;
use defmt::{error, info};
use embassy_net::Stack;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_rp::clocks::RoscRng;
use embassy_time::Timer;
use heapless::String;
use heapless::Vec;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};
use reqwless::request::Method;

use crate::config::ProxyConfig;
use crate::config::TflApiRequestConfig;
use crate::models::prediction::{ARRAY_MAX_SIZE_PREDICTION_MODEL, Prediction};

use static_cell::StaticCell;

// Static buffers for TLS client
static TLS_READ_BUF: StaticCell<[u8; 24576]> = StaticCell::new();
static TLS_WRITE_BUF: StaticCell<[u8; 16640]> = StaticCell::new();
static HTTP_RX_BUF: StaticCell<[u8; 16384]> = StaticCell::new();
static TCP_STATE: StaticCell<TcpClientState<1, 24576, 4096>> = StaticCell::new();

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn request_task(stack: Stack<'static>) {
    let mut rng: RoscRng = RoscRng;
    let mut sleep_this_cycle: bool = false;

    let tls_read_buffer = TLS_READ_BUF.init([0; 24576]);
    let tls_write_buffer = TLS_WRITE_BUF.init([0; 16640]);
    let rx_buffer = HTTP_RX_BUF.init([0; 16384]);
    let client_state = TCP_STATE.init(TcpClientState::<1, 24576, 4096>::new());

    loop {
        // Sleep for a while before the starting requests
        // N.B this is performed at the top of the loop, to ensure any allocated resources are dropped before sleeping
        if sleep_this_cycle {
            let query_delay_secs: u64 = option_env!("QUERY_DELAY")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);
            info!(
                "{}: Waiting for {} seconds before making the request...",
                function_name!(),
                query_delay_secs
            );
            Timer::after_secs(query_delay_secs).await;
        }

        // Clear static buffers
        rx_buffer.fill(0);
        tls_read_buffer.fill(0);
        tls_write_buffer.fill(0);

        // Create the HTTP client and DNS client
        info!("{}: Creating HTTP client and DNS client", function_name!());
        let tcp_client = TcpClient::new(stack, &client_state);
        let dns_client = DnsSocket::new(stack);
        let seed = rng.next_u64();

        let tls_config = TlsConfig::new(seed, tls_read_buffer, tls_write_buffer, TlsVerify::None);
        let mut http_client = HttpClient::new_with_tls(&tcp_client, &dns_client, tls_config);

        // Make the API requests
        info!("{}: Making API request", function_name!());

        // Request station & platform arrival predictions
        if let Some(predictions) = request_prediction(&mut http_client, rx_buffer).await {
            for prediction in predictions {
                info!("{}: Prediction: {}", function_name!(), prediction);
            }
        }

        // Set the flag to sleep at the start of the next cycle
        sleep_this_cycle = true;
    }
}

#[named]
async fn request_prediction<const RX_SZ: usize, const TX_SZ: usize>(
    http_client: &mut HttpClient<'_, TcpClient<'_, 1, RX_SZ, TX_SZ>, DnsSocket<'_>>,
    rx_buffer: &mut [u8],
) -> Option<Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>> {
    // define the URL for the TFL API request
    let tfl_api_request_config = TflApiRequestConfig::new();
    let proxy_config = ProxyConfig::new();
    let mut url_buffer: String<256> = String::new();
    let url = match write!(
        &mut url_buffer,
        "{}/StopPoint/{}/Arrivals?api_key={}",
        proxy_config.http_proxy,
        tfl_api_request_config.stopcode,
        tfl_api_request_config.api_primary_key
    ) {
        Ok(_) => url_buffer.as_str(),
        Err(e) => {
            error!(
                "{}: URL generation failed: Stack buffer size of 256 bytes was too small!: {}",
                function_name!(),
                e
            );
            None?
        }
    };

    // Make the HTTP request to the TFL API
    info!("{}: connecting to {}", function_name!(), &url);

    // Make HTTP request
    let mut request = match http_client.request(Method::GET, &url).await {
        Ok(req) => req,
        Err(e) => {
            error!("{}: Failed to make HTTP request: {}", function_name!(), e);
            None?
        }
    };

    // Send HTTP request
    let response = match request.send(rx_buffer).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("{}: Failed to send HTTP request: {}", function_name!(), e);
            None?
        }
    };

    // Read response body
    let body = match response.body().read_to_end().await {
        Ok(body) => body,
        Err(_) => {
            error!("{}: Failed to read response body", function_name!());
            return None;
        }
    };

    // Process JSON objects in body
    info!(
        "{}: About to deserialize payload. Total bytes in body variable: {}",
        function_name!(),
        body.len()
    );
    match serde_json_core::de::from_slice::<Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>>(&body)
    {
        Ok((mut predictions, _used)) => {
            info!(
                "{}: Successfully deserialized {} predictions",
                function_name!(),
                predictions.len()
            );

            if predictions.is_empty() {
                error!(
                    "{}: API returned a valid JSON array, but it was empty!",
                    function_name!()
                );
                return None;
            }

            // Filter only for platform of interest
            predictions.retain(|p| {
                p.platform_name
                    .contains(tfl_api_request_config.platform_name)
            });

            // Sort array by which is arriving first
            predictions.sort_unstable_by_key(|p| p.time_to_station);

            Some(predictions)
        }
        Err(e) => {
            error!(
                "{}: Deserialisation failed with error: {:?}",
                function_name!(),
                defmt::Debug2Format(&e)
            );
            return None;
        }
    }
}
