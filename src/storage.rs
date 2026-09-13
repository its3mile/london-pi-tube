use defmt::{error, info};
use embassy_rp::flash::{Async, Flash};
use embassy_rp::peripherals::FLASH;
use sequential_storage::cache::Cache;
use sequential_storage::map::{MapConfig, MapStorage};

pub const FLASH_SIZE: usize = 16 * 1024 * 1024;
const STORAGE_SIZE: u32 = 64 * 1024;
const STORAGE_RANGE: core::ops::Range<u32> =
    (FLASH_SIZE as u32 - STORAGE_SIZE)..(FLASH_SIZE as u32);
const STARTUP_COUNTER_KEY: u8 = 0;

pub async fn run_startup_test(flash: Flash<'_, FLASH, Async, FLASH_SIZE>) {
    let mut storage = MapStorage::new(
        flash,
        const { MapConfig::new(STORAGE_RANGE) },
        Cache::new_uncached(),
    );
    let mut data_buffer = [0u8; 16];

    let counter = match storage
        .fetch_item::<u32>(&mut data_buffer, &STARTUP_COUNTER_KEY)
        .await
    {
        Ok(Some(counter)) => counter,
        Ok(None) => 0,
        Err(_) => {
            error!("Persistent storage read failed");
            return;
        }
    };

    let next_counter = counter.wrapping_add(1);
    if storage
        .store_item(&mut data_buffer, &STARTUP_COUNTER_KEY, &next_counter)
        .await
        .is_err()
    {
        error!("Persistent storage write failed");
        return;
    }

    info!("Persistent storage startup counter: {}", next_counter);
}
