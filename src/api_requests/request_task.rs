use ::function_name::named;
use defmt::info;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_net::Stack;
use embassy_rp::clocks::RoscRng;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::Timer;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};

use crate::api_requests::*;

use crate::TFL_API_DISRUPTION_CHANNEL_SIZE;
use crate::TFL_API_PREDICTION_CHANNEL_SIZE;

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn request_task(
    stack: Stack<'static>,
    tfl_api_prediction_channel_sender: Sender<
        'static,
        ThreadModeRawMutex,
        models::prediction::Prediction,
        TFL_API_PREDICTION_CHANNEL_SIZE,
    >,
    tfl_api_status_channel_sender: Sender<
        'static,
        ThreadModeRawMutex,
        models::status::Status,
        TFL_API_DISRUPTION_CHANNEL_SIZE,
    >,
) {
    let mut rng: RoscRng = RoscRng;
    let mut sleep_this_cycle: bool = false;
    loop {
        // Sleep for a while before the starting requests
        // N.B this is performed at the top of the loop, to ensure any allocated resources are dropped before sleeping
        if sleep_this_cycle {
            let query_delay_secs: u64 = option_env!("QUERY_DELAY").and_then(|s| s.parse().ok()).unwrap_or(30);
            info!(
                "{}: Waiting for {} seconds before making the request...",
                function_name!(),
                query_delay_secs
            );
            Timer::after_secs(query_delay_secs).await;
        }

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

        // Make the API requests
        info!("{}: Making API requests", function_name!());

        // Request line status,
        if let Some(status) = request_status::request_status(&mut http_client, &mut rx_buffer).await {
            if !tfl_api_status_channel_sender.is_full() {
                info!("{}: Sending status to display task data channel", function_name!());
                tfl_api_status_channel_sender.send(status).await;
                info!("{}: Sent body to display task data channel", function_name!());
            }
        }

        // Request station & platform arrival predictions
        if let Some(predictions) = request_prediction::request_prediction(&mut http_client, &mut rx_buffer).await {
            for prediction in predictions {
                if !tfl_api_prediction_channel_sender.is_full() {
                    info!("{}: Sending predictions to display task data channel", function_name!());
                    tfl_api_prediction_channel_sender.send(prediction).await;
                    info!("{}: Sent body to display task data channel", function_name!());
                }
            }
        }

        // Set the flag to sleep at the start of the next cycle
        sleep_this_cycle = true;
    }
}
