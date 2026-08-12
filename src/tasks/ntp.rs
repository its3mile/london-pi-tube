//! NTP (client) task
//!
//! This task is responsible for determining the current time.
//!

use ::function_name::named;
use core::net::IpAddr;
use core::net::SocketAddr;
use defmt::*;
use defmt_rtt as _;
use embassy_net::Stack;
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket as EmbassyUdpSocket};
use embassy_time::{Duration, Timer};
use sntpc::{NtpContext, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use sntpc_time_embassy::EmbassyTimestampGenerator;

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn ntp_task(stack: Stack<'static>) {
    let ntp_delay = Duration::from_secs(60);

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
        let server_addr = match stack.dns_query("pool.ntp.org", DnsQueryType::A).await {
            Ok(mut results) => {
                if let Some(ip) = results.pop() {
                    match ip {
                        embassy_net::IpAddress::Ipv4(addr) => {
                            SocketAddr::new(IpAddr::V4(addr), 123u16)
                        }
                    }
                } else {
                    info!("{}: no DNS response for pool.ntp.org", function_name!());
                    Timer::after(ntp_delay).await;
                    continue;
                }
            }
            Err(err) => {
                info!(
                    "{}: DNS lookup failed for pool.ntp.org: {:?}",
                    function_name!(),
                    err
                );
                Timer::after(ntp_delay).await;
                continue;
            }
        };

        match get_time(server_addr, &ntp_socket, ntp_context).await {
            Ok(time) => {
                let subseconds = u64::from(time.sec_fraction()) * 1_000_000 / u64::from(u32::MAX);
                info!(
                    "{}: NTP time received {}.{}",
                    function_name!(),
                    time.sec(),
                    subseconds
                );
            }
            Err(_) => {
                info!("{}: NTP request failed", function_name!());
            }
        }

        Timer::after(ntp_delay).await;
    }
}
