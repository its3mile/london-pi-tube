#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use ::function_name::named;
use assign_resources::assign_resources;
use cyw43::JoinOptions;
use cyw43::aligned_bytes;
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_net::{Config, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::spi;
use embassy_rp::spi::Spi;
use embassy_rp::{Peri, peripherals};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Delay;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::epd3in7::EPD3in7;
use epd_waveshare::prelude::WaveshareDisplay;
use heapless::{String, Vec};
use static_cell::StaticCell;

mod config;
mod models;
mod panic;
mod tasks;

use config::WifiConfig;

use crate::models::update::Update;
use crate::models::{
    TFL_API_FIELD_LONG_STR_SIZE, TFL_API_FIELD_SHORT_STR_SIZE, TFL_API_FIELD_STR_SIZE,
};
use crate::tasks::display::display_task;
use crate::tasks::request::request_task;

// Program metadata for `picotool info`.
// This isn't needed, but it's recommended to have these minimal entries.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

// Clock divider for the RM2 Module
// This differs for between the Raspberry Pi Pico 1w/2w and Pimoroni Pico Plus 2w
use fixed::FixedU32;
use fixed::types::extra::U8;
const CHIP_SPECIFIC_CLOCK_DIVIDER: FixedU32<U8> = RM2_CLOCK_DIVIDER;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH1>;});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<
        'static,
        cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>,
        cyw43::Cyw43439,
    >,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

assign_resources! {
    display_resources: DisplayResources {
        spi1: SPI1,
        pin_9: PIN_9,
        pin_8: PIN_8,
        pin_13: PIN_13,
        pin_12: PIN_12,
        pin_11: PIN_11,
        pin_10: PIN_10,
    }
    network_resources: NetworkResources {
        pio0: PIO0,
        dma_ch0: DMA_CH0,
        dma_ch1: DMA_CH1,
        pin_23: PIN_23,
        pin_24: PIN_24,
        pin_25: PIN_25,
        pin_29: PIN_29,
    }
}

// Static for communication between tasks
// This is prefered over channels and pubsub because:
// - No copies - static lifetime and mutex ensures data is always shared without copying
// - No channel update locks - channels act as queues, only the latest is ever needed,
// even with a channel size of 0, data must be read before it can be replaced
// - Slow display updates - full display updates take in the order of 3 seconds, there is no
// scenario where a pubsub works as partial updates are not currently possible and data from
// the API is unlikely to change in the interval of 30 seconds
// - Request tasks waits - the request task should not wait for the display to finish reading
// as the display should always display the latest data from the request task (no stale updates)
static UPDATE: Mutex<CriticalSectionRawMutex, Update> = Mutex::new(Update {
    arrivals: Vec::new(),
    last_updated_secs: String::<TFL_API_FIELD_STR_SIZE>::new(),
    line_name: String::<TFL_API_FIELD_STR_SIZE>::new(),
    line_status: String::<TFL_API_FIELD_SHORT_STR_SIZE>::new(),
    platform_name: String::<TFL_API_FIELD_STR_SIZE>::new(),
    station_name: String::<TFL_API_FIELD_LONG_STR_SIZE>::new(),
});

// Atomic signal for the request task to emit, and the display task to consume
// to know when there is new data to physically show.
static NOTIFY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[named]
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("{}: Starting main task...", function_name!());
    let p: embassy_rp::Peripherals = embassy_rp::init(Default::default());
    let split_p = split_resources!(p);
    let fw = aligned_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = aligned_bytes!("..//cyw43-firmware/43439A0_clm.bin");
    let nvram = aligned_bytes!("../cyw43-firmware/nvram_rp2040.bin");

    // Spawn the task to update the display with predictions
    info!("{}: Initialising display...", function_name!());

    // Setup display pins and SPI bus
    let pin_reset: Output<'_> = Output::new(split_p.display_resources.pin_12, Level::Low);
    let pin_cs = Output::new(split_p.display_resources.pin_9, Level::High);
    let pin_data_cmd: Output<'_> = Output::new(split_p.display_resources.pin_8, Level::Low);
    let pin_spi_sclk = split_p.display_resources.pin_10;
    let pin_spi_mosi = split_p.display_resources.pin_11;
    let pin_busy = Input::new(
        split_p.display_resources.pin_13,
        embassy_rp::gpio::Pull::None,
    );
    let mut display_config = spi::Config::default();
    const DISPLAY_FREQ: u32 = 4_000_000;
    display_config.frequency = DISPLAY_FREQ;
    display_config.phase = spi::Phase::CaptureOnFirstTransition;
    display_config.polarity = spi::Polarity::IdleLow;
    let spi_bus = Spi::new_blocking_txonly(
        split_p.display_resources.spi1,
        pin_spi_sclk,
        pin_spi_mosi,
        display_config,
    );
    let mut spi_device: ExclusiveDevice<
        Spi<'_, embassy_rp::peripherals::SPI1, spi::Blocking>,
        Output<'_>,
        Delay,
    > = ExclusiveDevice::new(spi_bus, pin_cs, Delay).expect("Display: SPI initalise error");
    // Setup the EPD driver
    let epd_driver = EPD3in7::new(
        &mut spi_device,
        pin_busy,
        pin_data_cmd,
        pin_reset,
        &mut Delay,
        None,
    )
    .expect("Display: eink initalise error"); // Force unwrap, as there is nothing that can be done if this errors out

    // Spawn display task
    spawner.spawn(unwrap!(display_task(epd_driver, spi_device)));

    // Allow display task to run and show splash before continuing setup
    Timer::after_millis(500).await;

    // Setup the CYW43 Wifi chip
    info!("{}: Initialising CYW43 Wifi chip...", function_name!());
    let pwr = Output::new(split_p.network_resources.pin_23, Level::Low);
    let cs = Output::new(split_p.network_resources.pin_25, Level::High);
    let mut pio = Pio::new(split_p.network_resources.pio0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        CHIP_SPECIFIC_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        split_p.network_resources.pin_24,
        split_p.network_resources.pin_29,
        embassy_rp::dma::Channel::new(split_p.network_resources.dma_ch0, Irqs),
        embassy_rp::dma::Channel::new(split_p.network_resources.dma_ch1, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::Performance)
        .await;

    let config = Config::dhcpv4(Default::default());

    // Generate random seed
    let mut rng: RoscRng = RoscRng;
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(unwrap!(net_task(runner)));
    let wifi_config = WifiConfig::new();
    loop {
        match control
            .join(
                wifi_config.ssid,
                JoinOptions::new(wifi_config.password.as_bytes()),
            )
            .await
        {
            Ok(_) => break,
            Err(err) => {
                info!("{}: join failed with status={}", function_name!(), err);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("{}: waiting for DHCP...", function_name!());
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("{}: DHCP is now up!", function_name!());

    info!("{}: waiting for link up...", function_name!());
    while !stack.is_link_up() {
        Timer::after_millis(500).await;
    }
    info!("{}: Link is up!", function_name!());

    info!("{}: waiting for stack to be up...", function_name!());
    stack.wait_config_up().await;
    info!("{}: Stack is up!", function_name!());

    info!("{}: Starting TFL API request task...", function_name!());

    // Spawn the task to get predictions from the TFL API
    spawner.spawn(unwrap!(request_task(stack.clone())));

    let blink_delay = Duration::from_millis(500);
    loop {
        // Keep the main task alive
        Timer::after(blink_delay).await;
        info!("{}: Main task is running...", function_name!());

        // Blink the onboard LED to show that the main task is alive
        control.gpio_set(0, true).await;
        Timer::after(blink_delay).await;
        control.gpio_set(0, false).await;
    }
}
