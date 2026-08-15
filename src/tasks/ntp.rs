//! NTP (client) task
//!
//! This task is responsible for determining the current time.
//!

use ::function_name::named;
use core::cell::RefCell;
use core::fmt::Write as _;
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
use heapless::String;
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

    pub fn current_london(&self) -> Option<String<32>> {
        match self.current_unix() {
            Some(t) => {
                let (hour, min, sec) = unix_to_london_time(t);
                let mut formatted_time = String::<32>::new();
                let _ = core::write!(&mut formatted_time, "{:02}:{:02}:{:02}", hour, min, sec);
                Some(formatted_time)
            }
            None => None,
        }
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

/// Converts a Unix timestamp (seconds) into London local (hour, minute, second).
/// Handles UK DST (GMT/BST) completely in `no_std` with zero allocations.
pub fn unix_to_london_time(unix_sec: u64) -> (u32, u32, u32) {
    // Constants for time math
    const SECS_PER_MIN: u64 = 60;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_DAY: u64 = 86400;

    // 1. Extract days and remaining seconds since Unix epoch (1970-01-01)
    let days_since_epoch = unix_sec / SECS_PER_DAY;
    let day_sec = unix_sec % SECS_PER_DAY;

    let hour = (day_sec / SECS_PER_HOUR) as u32;
    let minute = ((day_sec % SECS_PER_HOUR) / SECS_PER_MIN) as u32;
    let second = (day_sec % SECS_PER_MIN) as u32;

    // 2. Approximate year and month from days_since_epoch for DST checking
    // (A lightweight calculation sufficient for UK transition rules)
    let (year, month, day, weekday) = civil_from_days(days_since_epoch as i64);

    // 3. Determine if UK is currently in BST (UTC+1)
    // Last Sunday of March to Last Sunday of October
    let is_dst = is_uk_dst_raw(year, month, day, weekday, hour);

    let offset_hours = if is_dst { 1 } else { 0 };
    let local_hour = (hour + offset_hours) % 24;

    (local_hour, minute, second)
}

/// Helper: Check UK DST boundaries dynamically
fn is_uk_dst_raw(year: i32, month: u32, day: u32, _weekday: u32, hour_utc: u32) -> bool {
    // March (3) to October (10)
    if month > 3 && month < 10 {
        return true;
    }

    // Last Sunday of March
    if month == 3 {
        let last_sun = last_sunday_of_month(year, 3);
        if day > last_sun {
            return true;
        }
        if day == last_sun {
            return hour_utc >= 1;
        } // Changes at 01:00 UTC
        return false;
    }

    // Last Sunday of October
    if month == 10 {
        let last_sun = last_sunday_of_month(year, 10);
        if day < last_sun {
            return true;
        }
        if day == last_sun {
            return hour_utc < 1;
        } // Ends at 01:00 UTC
        return false;
    }

    false
}

/// Finds the date of the last Sunday for March (3) or October (10)
fn last_sunday_of_month(year: i32, month: u32) -> u32 {
    // Start from 31st and walk back to find Sunday
    let mut d = if month == 3 { 31 } else { 31 }; // Both March and Oct have 31 days
    loop {
        let (_, _, _, wd) = civil_from_days(ymd_to_days(year, month, d));
        if wd == 0 {
            // 0 = Sunday
            return d;
        }
        d -= 1;
    }
}

// --- Lightweight calendar math helpers (No alloc, no std) ---

fn ymd_to_days(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146097 + doe as i64 - 719468
}

fn civil_from_days(z: i64) -> (i32, u32, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let weekday = ((z + 3) % 7) as u32; // 0 = Sun, 1 = Mon...
    (y as i32, m, d, weekday)
}
