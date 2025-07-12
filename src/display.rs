use ::function_name::named;
use core::fmt::Write;
use defmt::info;
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
use heapless::String;
use profont::*;

use crate::string_utilities::{first_two_words, insert_linebreaks_inplace};
use crate::tfl_requests::response_models::{TflApiPreciction, TFL_API_FIELD_LONG_STR_SIZE};

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
pub async fn update_display_task(
    mut epd_driver: DisplayDriver,
    mut spi_device: DisplaySpiDevice,
    tfl_api_prediction_channel_receiver: Receiver<'static, ThreadModeRawMutex, TflApiPreciction, 1>,
) {
    // Create a Display buffer to draw on, specific for this ePaper
    info!("{}: Initialising display nbuffer", function_name!());
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
    let character_style = MonoTextStyle::new(&PROFONT_24_POINT, Color::Black);
    let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();
    let position = display.bounding_box().center();
    Text::with_text_style("its3mile/london-pi-tube", position, character_style, text_style)
        .draw(&mut display)
        .expect("Failed create text in display buffer");

    epd_driver
        .update_and_display_frame(&mut spi_device, &mut display.buffer(), &mut Delay)
        .expect("Display: Failed to update display with splash");

    info!("{}: Display updated with splash and ready for use", function_name!());

    loop {
        info!("{}: Waiting for prediction data on channel", function_name!());
        let mut prediction: TflApiPreciction = tfl_api_prediction_channel_receiver.receive().await;
        info!("{}: Received prediction data on channel", function_name!());

        // Prepare the display message
        // Clear the display
        display.clear(Color::White).ok();

        // Line
        prediction
            .line_name
            .push_str(" Line\n")
            .expect("Failed to format line name");
        let character_style = MonoTextStyle::new(&PROFONT_14_POINT, Color::Black);
        let text_style = TextStyleBuilder::new().alignment(Alignment::Left).build();
        let position = display.bounding_box().top_left + Point::new(10, 25);
        let next = Text::with_text_style(&prediction.line_name, position, character_style, text_style)
            .draw(&mut display)
            .expect("Failed create line name text in display buffer");

        // Station
        prediction
            .station_name
            .push_str("\n")
            .expect("Failed to format station name");
        let next = Text::with_text_style(&prediction.station_name, next, character_style, text_style)
            .draw(&mut display)
            .expect("Failed create station name text in display buffer");

        // Platform
        let _ = Text::with_text_style(&prediction.platform_name, next, character_style, text_style)
            .draw(&mut display)
            .expect("Failed create platform name text in display buffer");

        // Destination
        let destination_name = first_two_words(&prediction.destination_name);
        let character_style = MonoTextStyleBuilder::new()
            .font(&PROFONT_24_POINT)
            .text_color(Color::Black)
            .background_color(Color::White)
            .build();
        let position = display.bounding_box().top_left + Point::new(10, display.size().height as i32 / 2);
        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Left)
            .baseline(Baseline::Middle)
            .build();
        let _ = Text::with_text_style(&destination_name, position, character_style, text_style)
            .draw(&mut display)
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
        let position = display.bounding_box().top_left
            + Point::new(
                (display.size().width - display.size().width / 10) as i32,
                display.size().height as i32 / 2,
            );
        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Middle)
            .build();
        let _ = Text::with_text_style(&time_to_station, position, character_style, text_style)
            .draw(&mut display)
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
        let character_style = MonoTextStyle::new(&PROFONT_14_POINT, Color::Black);
        let position = display.bounding_box().top_left
            + Point::new(
                10,
                ((display.size().height / 2) + PROFONT_14_POINT.character_size.height * 2) as i32,
            );
        let text_style = TextStyleBuilder::new().alignment(Alignment::Left).build();
        let _ = Text::with_text_style(&current_location, position, character_style, text_style)
            .draw(&mut display)
            .expect("Failed create text in display buffer");

        // Perform display update
        epd_driver
            .update_and_display_frame(&mut spi_device, &mut display.buffer(), &mut Delay)
            .expect("Failed to update display with prediction");

        info!("{}: Display updated with prediction", function_name!());
    }
}
