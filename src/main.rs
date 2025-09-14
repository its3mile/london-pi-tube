#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use ::function_name::named;
use assign_resources::assign_resources;
use core::str;
use cyw43::JoinOptions;
use cyw43_pio::{PioSpi, DEFAULT_CLOCK_DIVIDER};
use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_net::{Config, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::spi;
use embassy_rp::spi::Spi;
use embassy_rp::{peripherals, Peri};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::{epd3in7::*, prelude::*};
use panic_probe as _;
use static_cell::StaticCell;

mod string_utilities;
use crate::api_requests::request_task::request_task;
use crate::display_task::display_task;
pub use crate::string_utilities::{first_two_words, insert_linebreaks_inplace};

mod api_requests;
mod display_task;

use crate::api_requests::models::crowding::Crowding;
use crate::api_requests::models::prediction::Prediction;
use crate::api_requests::models::status::Status;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::task(pool_size = 1)]
async fn cyw43_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>) -> ! {
    runner.run().await
}

#[embassy_executor::task(pool_size = 1)]
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
        pin_23: PIN_23,
        pin_24: PIN_24,
        pin_25: PIN_25,
        pin_29: PIN_29,
    }
}

// Limited to 3 predictions to fit on the display
const TFL_API_PREDICTION_CHANNEL_SIZE: usize = 3;

// Limited to 1 status as only one line is monitored
const TFL_API_DISRUPTION_CHANNEL_SIZE: usize = 1;

// Limited to 1 status as only one station is monitored
const TFL_API_CROWDING_CHANNEL_SIZE: usize = 1;

#[named]
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("{}: Starting main task...", function_name!());
    let peripherals: embassy_rp::Peripherals = embassy_rp::init(Default::default());
    let split_peripherals = split_resources!(peripherals);

    // Spawn the task to update the display with predictions
    info!("{}: Initialising display...", function_name!());

    // Setup display pins and SPI bus
    let pin_reset: Output<'_> = Output::new(split_peripherals.display_resources.pin_12, Level::Low);
    let pin_cs = Output::new(split_peripherals.display_resources.pin_9, Level::High);
    let pin_data_cmd: Output<'_> = Output::new(split_peripherals.display_resources.pin_8, Level::Low);
    let pin_spi_sclk = split_peripherals.display_resources.pin_10;
    let pin_spi_mosi = split_peripherals.display_resources.pin_11;
    let pin_busy = Input::new(split_peripherals.display_resources.pin_13, embassy_rp::gpio::Pull::None);

    let mut display_config = spi::Config::default();
    const DISPLAY_FREQ: u32 = 16_000_000;
    display_config.frequency = DISPLAY_FREQ;
    display_config.phase = spi::Phase::CaptureOnFirstTransition;
    display_config.polarity = spi::Polarity::IdleLow;

    let spi_bus = Spi::new_blocking_txonly(
        split_peripherals.display_resources.spi1,
        pin_spi_sclk,
        pin_spi_mosi,
        display_config,
    );
    let mut spi_device: ExclusiveDevice<Spi<'_, embassy_rp::peripherals::SPI1, spi::Blocking>, Output<'_>, Delay> =
        ExclusiveDevice::new(spi_bus, pin_cs, Delay).expect("Display: SPI initalise error");

    // Setup the EPD driver
    let epd_driver = EPD3in7::new(&mut spi_device, pin_busy, pin_data_cmd, pin_reset, &mut Delay, None)
        .expect("Display: eink initalize error"); // Force unwrap, as there is nothing that can be done if this errors out

    // Spawn the task to update the display with predictions and statuss
    static TFL_API_PREDICTION_CHANNEL: Channel<ThreadModeRawMutex, Prediction, TFL_API_PREDICTION_CHANNEL_SIZE> =
        Channel::new();
    static TFL_API_DISRUPTION_CHANNEL: Channel<ThreadModeRawMutex, Status, TFL_API_DISRUPTION_CHANNEL_SIZE> =
        Channel::new();
    static TFL_API_CROWDING_CHANNEL: Channel<ThreadModeRawMutex, Crowding, TFL_API_CROWDING_CHANNEL_SIZE> =
        Channel::new();
    unwrap!(spawner.spawn(display_task(
        epd_driver,
        spi_device,
        TFL_API_PREDICTION_CHANNEL.receiver(),
        TFL_API_DISRUPTION_CHANNEL.receiver(),
        TFL_API_CROWDING_CHANNEL.receiver(),
    )));

    // Setup the CYW43 Wifi chip
    info!("{}: Initialising CYW43 Wifi chip...", function_name!());
    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");
    let pwr = Output::new(split_peripherals.network_resources.pin_23, Level::Low);
    let cs = Output::new(split_peripherals.network_resources.pin_25, Level::High);
    let mut pio = Pio::new(split_peripherals.network_resources.pio0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        split_peripherals.network_resources.pin_24,
        split_peripherals.network_resources.pin_29,
        split_peripherals.network_resources.dma_ch0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(cyw43_task(runner)));

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
    let (stack, runner) = embassy_net::new(net_device, config, RESOURCES.init(StackResources::new()), seed);

    unwrap!(spawner.spawn(net_task(runner)));
    const WIFI_NETWORK: &'static str = env!("WIFI_NETWORK");
    const WIFI_PASSWORD: &'static str = env!("WIFI_PASSWORD");
    loop {
        match control
            .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
            .await
        {
            Ok(_) => break,
            Err(err) => {
                info!("{}: join failed with status={}", function_name!(), err.status);
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

    // Spawn the task to get predictions from the TFL API and send them to the display task
    info!("{}: Starting TFL API request task...", function_name!());
    unwrap!(spawner.spawn(request_task(
        stack.clone(),
        TFL_API_PREDICTION_CHANNEL.sender(),
        TFL_API_DISRUPTION_CHANNEL.sender(),
        TFL_API_CROWDING_CHANNEL.sender()
    )));

    let loop_delay = Duration::from_secs(59);
    let blink_delay = Duration::from_millis(500);
    loop {
        // Keep the main task alive
        Timer::after(loop_delay).await;
        info!("{}: Main task is running...", function_name!());

        // Blink the onboard LED to show that the main task is alive
        control.gpio_set(0, true).await;
        Timer::after(blink_delay).await;
        control.gpio_set(0, false).await;
    }
}
