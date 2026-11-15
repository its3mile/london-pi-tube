//! Battery monitoring task.
//!
//! Samples VSYS through the board's sense divider on GPIO 43 (ADC channel 3 on
//! the RP2350B) and publishes the result in [`BATTERY`]. The interpretation of
//! the voltage lives in `application::battery`, where it is host-tested.
//!
//! N.B. the charge level can only be measured while running from the battery.
//! With USB connected, VSYS is the USB supply (about 4.8 V) and the reading is
//! reported as `PowerState::External`.

use ::function_name::named;
use application::battery::{BatteryReading, Calibration};
use core::cell::Cell;
use defmt::{error, info};
use embassy_rp::adc::{Adc, Async, Channel};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::Timer;

/// The most recent battery reading, or `None` before the first measurement.
pub static BATTERY: Mutex<CriticalSectionRawMutex, Cell<Option<BatteryReading>>> =
    Mutex::new(Cell::new(None));

/// Raw-to-millivolts calibration for this board's VSYS sense pin.
///
/// PROVISIONAL: assumes VSYS was 4.84 V on USB when the pin read 815 raw. That
/// raw value was measured on this board, but the 4.84 V is from a different
/// board. Replace with two (raw, multimeter mV) pairs measured on this board,
/// one on battery and one on USB.
const CALIBRATION: Calibration = Calibration::one_point(815, 4840);

/// Samples averaged per measurement, to smooth out ADC noise and the brief
/// dips in VSYS while the Wi-Fi radio transmits.
const SAMPLES_PER_READING: u32 = 16;

/// Gap between the samples of one measurement.
const SAMPLE_SPACING_MS: u64 = 5;

/// Time between measurements. The battery drains over hours, so this only
/// needs to be frequent enough for the display to stay roughly current.
const READING_INTERVAL_SECS: u64 = 30;

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn battery_task(mut adc: Adc<'static, Async>, mut vsys: Channel<'static>) {
    loop {
        let mut sum: u32 = 0;
        let mut count: u32 = 0;
        for _ in 0..SAMPLES_PER_READING {
            if let Ok(sample) = adc.read(&mut vsys).await {
                sum += u32::from(sample);
                count += 1;
            }
            Timer::after_millis(SAMPLE_SPACING_MS).await;
        }

        if count > 0 {
            let raw = (sum / count) as u16;
            let reading = BatteryReading::from_adc(raw, &CALIBRATION);
            info!(
                "{}: raw={} ({} samples), vsys={} mV, {}",
                function_name!(),
                raw,
                count,
                reading.millivolts,
                reading.state
            );
            BATTERY.lock(|cell| cell.set(Some(reading)));
        } else {
            error!("{}: every ADC conversion failed", function_name!());
        }

        Timer::after_secs(READING_INTERVAL_SECS).await;
    }
}
