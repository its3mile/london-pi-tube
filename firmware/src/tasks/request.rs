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
use application::tfl::{parse_line_status, parse_predictions};
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
use static_cell::ConstStaticCell;

use crate::config::ProxyConfig;
use crate::config::TflApiRequestConfig;
use crate::models::{ARRAY_MAX_SIZE_PREDICTION_MODEL, Line, Prediction};
use crate::{NOTIFY, SCHEDULE, UPDATE};

/// TLS record read buffer. embedded-tls reads one record at a time, and an
/// encrypted record is at most 16,640 bytes (2^14 + 256), so anything above
/// that is headroom rather than throughput.
const TLS_READ_BUF_SIZE: usize = 32 * 1024;

/// TLS record write buffer. embedded-tls fills each outgoing record up to this
/// size minus `TLS_RECORD_OVERHEAD` (128), and TLS caps record plaintext at
/// 2^14 bytes, so do not exceed 2^14 + 128 or large writes become oversized
/// records that the server rejects (`record_overflow`).
const TLS_WRITE_BUF_SIZE: usize = (1 << 14) + embedded_tls::TLS_RECORD_OVERHEAD;

/// TCP receive buffer: bounds the window the server can send into before
/// waiting for an ACK. Responses are tens of KB, so keep this large.
const TCP_RX_BUF_SIZE: usize = 32 * 1024;

/// TCP transmit buffer: requests are a few hundred bytes of HTTP GET.
const TCP_TX_BUF_SIZE: usize = 4 * 1024;

/// HTTP response buffer, holding the headers and the whole body. Arrivals
/// responses are roughly 1 KB per listed train; a body that does not fit fails
/// with `BufferTooSmall`.
const HTTP_RX_BUF_SIZE: usize = 32 * 1024;

// Static buffers for the HTTP/TLS client, all in SRAM.
//
// These are `ConstStaticCell`s, initialised at compile time directly in the
// static. Do not switch to `StaticCell::init(value)`: that builds the value on
// the stack first, and unoptimised (debug) builds keep several copies of it
// there, which for buffers this size overflows the stack into the statics.
static HTTP_RX_BUF: ConstStaticCell<[u8; HTTP_RX_BUF_SIZE]> =
    ConstStaticCell::new([0; HTTP_RX_BUF_SIZE]);
static TLS_READ_BUF: ConstStaticCell<[u8; TLS_READ_BUF_SIZE]> =
    ConstStaticCell::new([0; TLS_READ_BUF_SIZE]);
static TLS_WRITE_BUF: ConstStaticCell<[u8; TLS_WRITE_BUF_SIZE]> =
    ConstStaticCell::new([0; TLS_WRITE_BUF_SIZE]);
// N.B. the generic order is <N, TX_SZ, RX_SZ>.
static TCP_STATE: ConstStaticCell<TcpClientState<1, TCP_TX_BUF_SIZE, TCP_RX_BUF_SIZE>> =
    ConstStaticCell::new(TcpClientState::new());

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn request_task(stack: Stack<'static>) {
    let mut rng: RoscRng = RoscRng;
    let mut sleep_this_cycle: bool = false;

    let rx_buffer = HTTP_RX_BUF.take();
    let tls_read_buffer = TLS_READ_BUF.take();
    let tls_write_buffer = TLS_WRITE_BUF.take();
    let client_state = TCP_STATE.take();

    loop {
        // Handle scheduled sleep
        SCHEDULE.wait_until_active().await;

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
                    update.line_name = predictions[0].line_name.clone();
                    update.platform_name = predictions[0].platform_name.clone();
                    update.station_name = predictions[0].station_name.clone();
                    update.arrivals = predictions;
                }
            }

            // Line Status with fallback logic
            if let Some(line) = fetched_status {
                if let Some(line_status) = line.line_status {
                    // Explicit warning/alert state from the API :(
                    update.line_status = line_status.status_severity_description;
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
async fn request_prediction<const TX_SZ: usize, const RX_SZ: usize>(
    http_client: &mut HttpClient<'_, TcpClient<'_, 1, TX_SZ, RX_SZ>, DnsSocket<'_>>,
    rx_buffer: &mut [u8],
) -> Option<Vec<Prediction, ARRAY_MAX_SIZE_PREDICTION_MODEL>> {
    // define the URL for the TFL API request
    let tfl_api_request_config = TflApiRequestConfig::new();
    let proxy_config = ProxyConfig::new();
    let mut url_buffer: String<256> = String::new();
    let url = match write!(
        &mut url_buffer,
        "{}/StopPoint/{}/Arrivals?app_key={}",
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
        Err(e) => {
            // `BufferTooSmall` means the response exceeded HTTP_RX_BUF_SIZE.
            error!("{}: Failed to read response body: {}", function_name!(), e);
            return None;
        }
    };

    // Process JSON objects in body
    info!(
        "{}: About to deserialize payload. Total bytes in body variable: {}",
        function_name!(),
        body.len()
    );
    // Filters by platform while parsing, so any number of arrivals across
    // the station's other platforms/lines is tolerated. Sorted soonest first.
    match parse_predictions::<ARRAY_MAX_SIZE_PREDICTION_MODEL>(
        body,
        tfl_api_request_config.platform_name,
    ) {
        Ok(predictions) if predictions.is_empty() => {
            warn!(
                "{}: No predictions for platform of interest",
                function_name!()
            );
            None
        }
        Ok(predictions) => {
            info!(
                "{}: Successfully deserialized {} predictions",
                function_name!(),
                predictions.len()
            );
            Some(predictions)
        }
        Err(e) => {
            error!(
                "{}: Deserialisation failed with error: {}",
                function_name!(),
                e
            );
            None
        }
    }
}

#[named]
pub async fn request_status<const TX_SZ: usize, const RX_SZ: usize>(
    http_client: &mut HttpClient<'_, TcpClient<'_, 1, TX_SZ, RX_SZ>, DnsSocket<'_>>,
    rx_buffer: &mut [u8],
) -> Option<Line> {
    // 1. Dynamic URL Generation mirroring request_prediction
    let tfl_api_request_config = TflApiRequestConfig::new();
    let proxy_config = ProxyConfig::new();
    let mut url_buffer: String<256> = String::new();

    // Hardcoded line ID parameters are replaced by runtime configs
    // Assuming tfl_api_request_config contains or can provide your target line ID
    let url = match write!(
        &mut url_buffer,
        "{}/Line/{}/Status?app_key={}",
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
        Err(e) => {
            // `BufferTooSmall` means the response exceeded HTTP_RX_BUF_SIZE.
            error!("{}: Failed to read response body: {}", function_name!(), e);
            return None;
        }
    };

    info!(
        "{}: About to deserialize payload. Total bytes in body variable: {}",
        function_name!(),
        body.len()
    );

    // 5. Process JSON objects in body
    match parse_line_status(body) {
        Ok(Some(line)) => {
            debug!("{}: line = {}", function_name!(), line);
            Some(line)
        }
        Ok(None) => {
            error!(
                "{}: API returned a valid JSON array, but it was empty!",
                function_name!()
            );
            None
        }
        Err(e) => {
            error!(
                "{}: Deserialisation failed with error: {}",
                function_name!(),
                e
            );
            // Helpful fallback log to spot payload issues in terminal
            info!(
                "{}: Raw response payload: {}",
                function_name!(),
                str::from_utf8(body).unwrap_or("[Malformed UTF-8 body]")
            );
            None
        }
    }
}
