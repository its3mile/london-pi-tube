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
use defmt::{debug, error, info, warn};
use embassy_net::Stack;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_rp::clocks::RoscRng;
use embassy_time::Timer;
use embassy_time::{Duration, with_timeout};
use heapless::String;
use heapless::Vec;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};
use reqwless::request::Method;

use crate::config::ProxyConfig;
use crate::config::TflApiRequestConfig;
use crate::models::prediction::{ARRAY_MAX_SIZE_PREDICTION_MODEL, Prediction};
use crate::models::status::{ARRAY_MAX_SIZE_LINE_STATUS_MODEL, Status};
use crate::{NOTIFY, UPDATE};

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

        // Request station & platform arrival predictions
        info!("{}: Making Prediction API request", function_name!());
        let fetched_predictions = match with_timeout(
            Duration::from_secs(10),
            request_prediction(&mut http_client, rx_buffer),
        )
        .await
        {
            Ok(Some(predictions)) => {
                debug!("{}: predictions = {}", function_name!(), predictions);
                Some(predictions)
            }
            Ok(None) => {
                error!("Predictions API returned an empty or unparsable payload");
                None
            }
            Err(_) => {
                error!("Predictions network request timed out!");
                None
            }
        };

        // Request (line) status (all okay, minor delays, ...)
        info!("{}: Making Status API request", function_name!());
        let fetched_status = match with_timeout(
            Duration::from_secs(10),
            request_status(&mut http_client, rx_buffer),
        )
        .await
        {
            Ok(Some(status)) => {
                debug!("{}: status = {}", function_name!(), status);
                Some(status)
            }
            Ok(None) => {
                error!("Status API returned an empty or unparsable payload");
                None
            }
            Err(_) => {
                error!("Status network request timed out!");
                None
            }
        };

        // Trigger an update if there are predictions, or to confirm status
        {
            let mut update = UPDATE.lock().await;

            // Update predictions data if available
            if let Some(predictions) = fetched_predictions {
                if !predictions.is_empty() {
                    update.last_updated_secs = predictions[0].timestamp.clone();
                    update.line_name = predictions[0].line_name.clone();
                    update.platform_name = predictions[0].platform_name.clone();
                    update.station_name = predictions[0].station_name.clone();

                    update.arrivals = predictions;
                }
            }

            // Line Status with fallback logic
            if let Some(status) = fetched_status {
                if let Some(line_status) = status.line_statuses.first() {
                    // Explicit warning/alert state from the API :(
                    update.line_status = line_status.status_severity_description.clone();
                } else {
                    // No status data returned = everything is running perfectly fine!
                    update.line_status = String::try_from("Good Service").unwrap_or_default();
                }
            }
        }

        // Signal the display task that data is ready
        NOTIFY.signal(());

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

            if predictions.is_empty() {
                warn!(
                    "{}: No predictions retained after filtering for platform on interest",
                    function_name!()
                );
                return None;
            }

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

#[named]
pub async fn request_status<const RX_SZ: usize, const TX_SZ: usize>(
    http_client: &mut HttpClient<'_, TcpClient<'_, 1, RX_SZ, TX_SZ>, DnsSocket<'_>>,
    rx_buffer: &mut [u8],
) -> Option<Status> {
    // 1. Dynamic URL Generation mirroring request_prediction
    let tfl_api_request_config = TflApiRequestConfig::new();
    let proxy_config = ProxyConfig::new();
    let mut url_buffer: String<256> = String::new();

    // Hardcoded line ID parameters are replaced by runtime configs
    // Assuming tfl_api_request_config contains or can provide your target line ID
    let url = match write!(
        &mut url_buffer,
        "{}/Line/{}/Status?api_key={}",
        proxy_config.http_proxy,
        tfl_api_request_config.line_id,
        tfl_api_request_config.api_primary_key
    ) {
        Ok(_) => url_buffer.as_str(),
        Err(e) => {
            error!(
                "{}: URL generation failed: Stack buffer size of 256 bytes was too small!: {}",
                function_name!(),
                e
            );
            return None;
        }
    };

    // 2. Make the HTTP request to the TFL API
    info!("{}: connecting to {}", function_name!(), &url);

    let mut request = match http_client.request(Method::GET, &url).await {
        Ok(req) => req,
        Err(e) => {
            error!("{}: Failed to make HTTP request: {}", function_name!(), e);
            return None;
        }
    };

    // 3. Send HTTP request
    let response = match request.send(rx_buffer).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("{}: Failed to send HTTP request: {}", function_name!(), e);
            return None;
        }
    };

    // 4. Read response body
    let body = match response.body().read_to_end().await {
        Ok(body) => body,
        Err(_) => {
            error!("{}: Failed to read response body", function_name!());
            return None;
        }
    };

    info!(
        "{}: About to deserialize payload. Total bytes in body variable: {}",
        function_name!(),
        body.len()
    );

    // 5. Process JSON objects in body
    match serde_json_core::de::from_slice::<Vec<Status, ARRAY_MAX_SIZE_LINE_STATUS_MODEL>>(&body) {
        Ok((mut statuses, _used)) => {
            info!(
                "{}: Successfully deserialized {} line statuses",
                function_name!(),
                statuses.len()
            );
            debug!("{}: statuses = {}", function_name!(), statuses);
            // Instead of pop() which could panic if empty, safely handle it
            if statuses.is_empty() {
                error!(
                    "{}: API returned a valid JSON array, but it was empty!",
                    function_name!()
                );
                return None;
            }

            // Safely take the last element out of the heapless::Vec
            let status = statuses.pop();
            status
        }
        Err(e) => {
            error!(
                "{}: Deserialisation failed with error: {:?}",
                function_name!(),
                defmt::Debug2Format(&e)
            );

            // Helpful fallback log to spot payload issues in terminal
            info!(
                "{}: Raw response payload: {}",
                function_name!(),
                str::from_utf8(body).unwrap_or("[Malformed UTF-8 body]")
            );
            return None;
        }
    }
}
