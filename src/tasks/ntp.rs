//! NTP (client) task
//!
//! This task is responsible for determining the current time.
//!

use ::function_name::named;
use core::cell::RefCell;
use core::net::IpAddr;
use core::net::SocketAddr;
use defmt::*;
use defmt_rtt as _;
use embassy_net::Stack;
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket as EmbassyUdpSocket};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant, Timer};
use sntpc::{NtpContext, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use sntpc_time_embassy::EmbassyTimestampGenerator;

// NTP server to get time information from
const NTP_SERVER: &str = "pool.ntp.org";

// Static Wall Clock
// Access is thread & core safe via a blocking CriticalSectionRawMutex
// Blocking is preferred as the operations on it have deterministic timing and allow for access in non-async contexts
pub static WALL_CLOCK: Mutex<CriticalSectionRawMutex, RefCell<WallClock>> =
    Mutex::new(RefCell::new(WallClock::new()));

///
pub struct WallClock {
    base_unix_time: u64,
    anchor_instant: Instant,
}

impl WallClock {
    const fn new() -> Self {
        Self {
            // The unix time
            base_unix_time: 0,
            // The current time
            anchor_instant: Instant::from_ticks(0),
        }
    }

    /// Update time with new NTP sync
    fn update(&mut self, unix_time: u64) {
        self.base_unix_time = unix_time;
        self.anchor_instant = Instant::now();
    }

    /// Calculate wall time
    pub fn current_unix(&self) -> Option<u64> {
        // Not synced yet
        if self.base_unix_time == 0 {
            return None;
        }
        let elapsed_secs = self.anchor_instant.elapsed().as_secs();
        Some(self.base_unix_time + elapsed_secs)
    }
}

// Re-sync local time with NTP server every 2 hours
const RESYNC_DURATION: Duration = Duration::from_secs(7200);

// Request retry delay to NTP server
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn ntp_task(stack: Stack<'static>) {
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buffer = [0u8; 256];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0u8; 256];
    let mut ntp_socket = EmbassyUdpSocket::new(
        stack.clone(),
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    ntp_socket.bind(0).expect("NTP socket bind failed");
    let ntp_socket = UdpSocketWrapper::new(ntp_socket);

    loop {
        let ntp_context = NtpContext::new(EmbassyTimestampGenerator::default());
        let server_addr = match stack.dns_query(NTP_SERVER, DnsQueryType::A).await {
            Ok(mut results) => {
                if let Some(ip) = results.pop() {
                    match ip {
                        embassy_net::IpAddress::Ipv4(addr) => {
                            SocketAddr::new(IpAddr::V4(addr), 123u16)
                        }
                    }
                } else {
                    info!("{}: no DNS response for {}", function_name!(), NTP_SERVER);
                    Timer::after(RETRY_DELAY).await;
                    continue;
                }
            }
            Err(err) => {
                error!(
                    "{}: DNS lookup failed for {}: {:?}",
                    function_name!(),
                    NTP_SERVER,
                    err
                );
                Timer::after(RETRY_DELAY).await;
                continue;
            }
        };

        match get_time(server_addr, &ntp_socket, ntp_context).await {
            Ok(fetched_time) => {
                let fetched_unix_time = fetched_time.sec();
                let fetched_unix_time_subseconds =
                    u64::from(fetched_time.sec()) * 1_000_000 / u64::from(u32::MAX);
                info!(
                    "{}: NTP time received {}.{}",
                    function_name!(),
                    fetched_unix_time,
                    fetched_unix_time_subseconds
                );

                WALL_CLOCK.lock(|cell| {
                    // cell is the RefCell, borrow_mut() gives us the &mut WallClock
                    let mut clock = cell.borrow_mut();
                    clock.update(fetched_unix_time);
                });
            }
            Err(_) => {
                error!("{}: NTP request failed", function_name!());
                Timer::after(RETRY_DELAY).await;
                continue;
            }
        }

        // Delay task until resync delay
        Timer::after(RESYNC_DURATION).await;
    }
}
