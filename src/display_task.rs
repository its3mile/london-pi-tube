use crate::api_requests::models::prediction::Prediction;
use crate::api_requests::models::status::Status;

use crate::api_requests::models::TFL_API_FIELD_LONG_STR_SIZE;
use crate::string_utilities::{first_two_words, insert_linebreaks_inplace, split_iso8601_timestamp};
use crate::{TFL_API_DISRUPTION_CHANNEL_SIZE, TFL_API_PREDICTION_CHANNEL_SIZE};
use ::function_name::named;
use core::fmt::Write;
use defmt::info;
use defmt_rtt as _;
use defmt_rtt as _;
use embassy_rp::gpio::{Input, Output};
use embassy_rp::spi;
use embassy_rp::spi::Spi;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::Delay;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::text::Baseline;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::PrimitiveStyle,
    text::{Alignment, Text, TextStyleBuilder},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::{epd3in7::*, prelude::*};
use heapless::{String, Vec};
use panic_probe as _;
use profont::*;
pub type DisplayDriver = EPD3in7<
    ExclusiveDevice<Spi<'static, embassy_rp::peripherals::SPI1, spi::Blocking>, Output<'static>, Delay>,
    Input<'static>,
    Output<'static>,
    Output<'static>,
    Delay,
>;

pub type DisplaySpiDevice =
    ExclusiveDevice<Spi<'static, embassy_rp::peripherals::SPI1, spi::Blocking>, Output<'static>, Delay>;

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn display_task(
    mut epd_driver: DisplayDriver,
    mut spi_device: DisplaySpiDevice,
    prediction_receiver: Receiver<'static, ThreadModeRawMutex, Prediction, TFL_API_PREDICTION_CHANNEL_SIZE>,
    status_receiver: Receiver<'static, ThreadModeRawMutex, Status, TFL_API_DISRUPTION_CHANNEL_SIZE>,
) {
    // Create a Display buffer to draw on, specific for this ePaper
    info!("{}: Initialising display buffer", function_name!());
    let mut display = Display3in7::default();

    // Landscape mode, USB plug to the right
    display.set_rotation(DisplayRotation::Rotate270);

    // Change the background from the default black to white
    let _ = display
        .bounding_box()
        .into_styled(PrimitiveStyle::with_fill(Color::White))
        .draw(&mut display);

    // Clear the display buffer
    info!("{}: Clearing display buffer", function_name!());
    display.clear(Color::White).ok();

    // Render splash drawing
    info!("{}: Rendering splash drawing", function_name!());
    make_splash(&mut display);
    epd_driver
        .update_and_display_frame(&mut spi_device, &mut display.buffer(), &mut Delay)
        .expect("Display: Failed to update display with splash");

    info!("{}: Display updated with splash and ready for use", function_name!());

    loop {
        // Wait for status data from the channel
        info!("{}: Waiting for status data on channel", function_name!());
        let status: Status = status_receiver.receive().await;
        info!("{}: Received status data on channel", function_name!());
        info!("{}: {}", function_name!(), status);

        // Wait for prediction data from the channel
        info!("{}: Waiting for prediction data on channel", function_name!());
        prediction_receiver.ready_to_receive().await;
        let mut predictions = Vec::<Prediction, TFL_API_PREDICTION_CHANNEL_SIZE>::new();
        while match prediction_receiver.try_receive() {
            Ok(prediction) => {
                info!("{}: Received prediction data on channel", function_name!());
                predictions.push(prediction).ok();
                true
            }
            Err(_) => {
                info!("{}: No additional prediction data on channel", function_name!());
                false
            }
        } {}

        // Prepare the display message
        // Clear the display
        display.clear(Color::White).ok();

        // Format header
        // This is the line name, line status, station name and platform name
        let header_position = display.bounding_box().top_left + Point::new(10, 15);
        let line_name = predictions[0].line_name.as_str();
        let line_status = status.line_statuses[0].status_severity_description.as_str();
        let station_name = predictions[0].station_name.as_str();
        let platform_name = predictions[0].platform_name.as_str();
        let mut next = make_header(
            &mut display,
            header_position,
            line_name,
            line_status,
            station_name,
            platform_name,
        );

        // Format body
        // This is the actual prediction information
        for prediction in &mut predictions {
            info!("{}: Processing prediction for display", function_name!());
            info!("{}: {}", function_name!(), prediction);
            next = next + Point::new(0, 15); // Add spacing from previous text
            next = make_body_object(&mut display, next, prediction);
        }

        // Format footer
        // This is the last update time
        let footer_position = display.bounding_box().top_left + Point::new(10, display.size().height as i32 - 10);
        let timestamp = predictions[0].timestamp.as_str();
        make_footer(&mut display, footer_position, timestamp);

        // Perform display update
        epd_driver
            .update_and_display_frame(&mut spi_device, &mut display.buffer(), &mut Delay)
            .expect("Failed to update display with prediction");

        info!("{}: Display updated with prediction", function_name!());

        // Clear the channels to prepare for next update
        info!("{}: Clearing data channels", function_name!());
        prediction_receiver.clear();
        status_receiver.clear();
        info!("{}: Data channels cleared", function_name!());
    }
}

fn make_splash(display: &mut Display3in7) {
    let character_style = MonoTextStyle::new(&PROFONT_24_POINT, Color::Black);
    let text_style: embedded_graphics::text::TextStyle = TextStyleBuilder::new().alignment(Alignment::Center).build();
    let position = display.bounding_box().center();
    Text::with_text_style("its3mile/london-pi-tube", position, character_style, text_style)
        .draw(display)
        .expect("Failed create text in display buffer");
}

fn make_header(
    display: &mut Display3in7,
    start: Point,
    line_name: &str,
    line_status: &str,
    station_name: &str,
    platform_name: &str,
) -> Point {
    // Line & Line Status
    // Station
    // Platform
    let mut header = String::<TFL_API_FIELD_LONG_STR_SIZE>::new();
    let _ = write!(
        &mut header,
        "{} Line - {}\n{}\n{}\n",
        line_name, line_status, station_name, platform_name
    );

    let character_style = MonoTextStyle::new(&PROFONT_14_POINT, Color::Black);
    let text_style = TextStyleBuilder::new().alignment(Alignment::Left).build();
    let position = start;
    Text::with_text_style(&header, position, character_style, text_style)
        .draw(display)
        .expect("Failed create line name text in display buffer")
}

fn make_body_object(display: &mut Display3in7, start: Point, prediction: &Prediction) -> Point {
    // Vehicle ID and Destination
    let destination_name = first_two_words(&prediction.destination_name);
    let mut vehicle_id_and_destination = String::<TFL_API_FIELD_LONG_STR_SIZE>::new();
    let _ = write!(
        &mut vehicle_id_and_destination,
        "({}) {}\n",
        prediction.vehicle_id, destination_name
    );
    let character_style = MonoTextStyleBuilder::new()
        .font(&PROFONT_24_POINT)
        .text_color(Color::Black)
        .background_color(Color::White)
        .build();
    let position = start;
    let text_style = TextStyleBuilder::new()
        .alignment(Alignment::Left)
        .baseline(Baseline::Middle)
        .build();
    let next = Text::with_text_style(&vehicle_id_and_destination, position, character_style, text_style)
        .draw(display)
        .expect("Failed create text in display buffer");

    // Time to station
    let mut time_to_station = String::<16>::new();
    if (prediction.time_to_station as f32 / 60.0) < 1.0 {
        let _ = write!(&mut time_to_station, "< 1 min");
    } else if (prediction.time_to_station as f32 / 60.0) < 2.0 {
        let _ = write!(&mut time_to_station, "< 2 mins");
    } else {
        let _ = write!(&mut time_to_station, "{} mins", prediction.time_to_station / 60);
    }
    let character_style = MonoTextStyleBuilder::new()
        .font(&PROFONT_24_POINT)
        .text_color(Color::Black)
        .background_color(Color::White)
        .build();
    let position = start + Point::new((display.size().width - display.size().width / 10) as i32, 0);
    let text_style = TextStyleBuilder::new()
        .alignment(Alignment::Right)
        .baseline(Baseline::Middle)
        .build();
    let _ = Text::with_text_style(&time_to_station, position, character_style, text_style)
        .draw(display)
        .expect("Failed create text in display buffer");

    // Current location
    let mut current_location = String::<TFL_API_FIELD_LONG_STR_SIZE>::new();
    current_location
        .push_str("Current Location: ")
        .expect("Failed to format current location");
    current_location
        .push_str(prediction.current_location.as_str())
        .expect("Failed to format current location");
    insert_linebreaks_inplace(
        &mut current_location,
        ((display.size().width / PROFONT_14_POINT.character_size.width) - 2) as usize,
    );
    current_location
        .push_str("\n")
        .expect("Failed to format current location");
    let character_style = MonoTextStyle::new(&PROFONT_14_POINT, Color::Black);
    let text_style = TextStyleBuilder::new().alignment(Alignment::Left).build();
    Text::with_text_style(&current_location, next, character_style, text_style)
        .draw(display)
        .expect("Failed create text in display buffer")
}

fn make_footer(display: &mut Display3in7, start: Point, last_update_iso8601_ts: &str) {
    // Last Update Time
    let (_, time) = split_iso8601_timestamp(last_update_iso8601_ts);
    let mut footer = String::<TFL_API_FIELD_LONG_STR_SIZE>::new();
    let _ = write!(&mut footer, "Last updated at: {}", time);
    let character_style = MonoTextStyle::new(&PROFONT_14_POINT, Color::Black);
    let text_style = TextStyleBuilder::new().alignment(Alignment::Left).build();
    let position = start;
    let _ = Text::with_text_style(&footer, position, character_style, text_style)
        .draw(display)
        .expect("Failed create last update time text in display buffer");
}
