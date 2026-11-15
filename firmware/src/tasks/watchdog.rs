//! Hardware watchdog: restart the device if the firmware stops making progress.
//!
//! The panic handler restarts the device after a panic, but a hang is not a
//! panic. The e-paper driver, for example, waits on the panel's busy pin in a
//! blocking loop with no timeout and never yields, so a panel that stops
//! responding freezes every task (issue #19). Nobody can reset the device by
//! hand once it is installed.
//!
//! This task feeds the watchdog every [`FEED_INTERVAL`]. It only gets to run
//! while the executor is running tasks, so anything that blocks the executor
//! for longer than [`WATCHDOG_TIMEOUT`] stops the feeding and the watchdog
//! resets the chip.

use ::function_name::named;
use defmt::{info, warn};
use embassy_rp::watchdog::{ResetReason, Watchdog};
use embassy_time::{Duration, Timer};

/// Time without a feed before the chip is reset.
///
/// Must comfortably exceed the longest legitimate blocking operation, a full
/// e-paper refresh including waking the panel (about 2 s), and stay under the
/// RP2350's maximum of about 16.7 s.
pub const WATCHDOG_TIMEOUT: Duration = Duration::from_secs(8);

/// How often the watchdog is fed: a few feeds per timeout, so that ordinary
/// delays in scheduling this task never cause a reset.
const FEED_INTERVAL: Duration = Duration::from_secs(2);

/// Log why the previous boot ended, if the watchdog caused it.
///
/// `Forced` is the panic handler restarting the device after a panic;
/// `TimedOut` is either a hang caught by this task, or the panic handler
/// itself getting stuck.
#[named]
pub fn log_reset_reason(watchdog: &Watchdog) {
    match watchdog.reset_reason() {
        Some(ResetReason::TimedOut) => warn!(
            "{}: previous boot ended in a watchdog timeout (firmware hang)",
            function_name!()
        ),
        Some(ResetReason::Forced) => info!(
            "{}: previous boot ended in a forced restart (after a panic)",
            function_name!()
        ),
        None => info!("{}: normal power-on or external reset", function_name!()),
    }
}

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn watchdog_task(mut watchdog: Watchdog) -> ! {
    // Pause while a debugger has the cores halted, so breakpoints and
    // flashing do not reset the chip.
    watchdog.pause_on_debug(true);
    watchdog.start(WATCHDOG_TIMEOUT);
    info!(
        "{}: started, timeout {} ms",
        function_name!(),
        WATCHDOG_TIMEOUT.as_millis()
    );

    loop {
        Timer::after(FEED_INTERVAL).await;
        watchdog.feed(WATCHDOG_TIMEOUT);
    }
}
