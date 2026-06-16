//! Display task
//!
//! This draws and renders the sent update onto the display,
//! to look something like this:
//!
//! +-------------------------------------------------------------+
//! | District Line - East Putney Underground Station             |
//! | Eastbound -  Platform 1                                     |
//! |                                                             |
//! |  2 mins      Barking                                        |
//! |              Approaching Southfields                        |
//! |                                                             |
//! |  7 mins      Upminster                                      |
//! |                                                             |
//! |                                                             |
//! | Good Service                                 Updated: 15:43 |
//! +-------------------------------------------------------------+
//!

use ::function_name::named;
use core::fmt::Write;
use defmt::{error, info};
use embassy_rp::gpio::{Input, Output};
use embassy_rp::spi;
use embassy_rp::spi::Spi;
use embassy_time::Delay;
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_iconoir::prelude::*;
use epd_waveshare::color::Color;
use epd_waveshare::epd3in7::Display3in7;
use epd_waveshare::prelude::WaveshareDisplay;
use heapless::String;
use u8g2_fonts::{
    FontRenderer, fonts,
    types::{FontColor, HorizontalAlignment, VerticalPosition},
};

use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::{epd3in7::*, prelude::*};

use crate::models::update::Update;
use crate::{NOTIFY, UPDATE};

/// The main display task that handles displaying sensor data and connection status
pub type DisplayDriver = EPD3in7<
    ExclusiveDevice<
        Spi<'static, embassy_rp::peripherals::SPI1, spi::Blocking>,
        Output<'static>,
        Delay,
    >,
    Input<'static>,
    Output<'static>,
    Output<'static>,
    Delay,
>;

pub type DisplaySpiDevice = ExclusiveDevice<
    Spi<'static, embassy_rp::peripherals::SPI1, spi::Blocking>,
    Output<'static>,
    Delay,
>;

#[named]
#[embassy_executor::task(pool_size = 1)]
pub async fn display_task(mut epd_driver: DisplayDriver, mut spi_device: DisplaySpiDevice) {
    // Create a Display buffer to draw on, specific for this ePaper
    info!("{}: Initialising display buffer", function_name!());
    let mut display = Display3in7::default();

    // Landscape mode, USB plug to the right
    display.set_rotation(DisplayRotation::Rotate270);

    // Render splash drawing
    info!("{}: Rendering splash drawing", function_name!());
    show_splash(&mut display, &mut epd_driver, &mut spi_device)
        .unwrap_or_else(|_| error!("{}: Failed to show splash", function_name!()));
    info!(
        "{}: Display updated with splash and ready for use",
        function_name!()
    );

    let styles = DisplayStyles::new();

    // Main update loop
    loop {
        // Sleep display
        epd_driver
            .sleep(&mut spi_device, &mut Delay)
            .expect("Display: Failed to put display to sleep.");

        // Acquire lock to read data update
        info!("{}: Wait for signal...", function_name!());
        NOTIFY.wait().await;

        // Wake display
        epd_driver
            .wake_up(&mut spi_device, &mut Delay)
            .expect("Display: Failed to wake display from sleep.");

        // Get update
        let update = {
            let update = UPDATE.lock().await;
            (*update).clone()
        }; // Release lock

        // Show update on display
        info!("{}: Signal received! Showing update...", function_name!());
        let _ = display
            .clear(styles.colors.bg)
            .map_err(|_| DisplayError::RenderingFailed);
        show_update(&mut display, &mut epd_driver, &mut spi_device, update)
            .unwrap_or_else(|_| error!("{}: Failed to show update", function_name!()));
        info!("{}: Finished rendering update", function_name!());
    }
}

/// Draw and render the splash to the epaper display
#[named]
fn show_splash(
    display: &mut Display3in7,
    epd_driver: &mut DisplayDriver,
    spi_device: &mut DisplaySpiDevice,
) -> Result<(), DisplayError> {
    let styles = DisplayStyles::new();

    info!("{}: Clearing display", function_name!());

    display
        .clear(styles.colors.bg)
        .map_err(|_| DisplayError::RenderingFailed)?;

    info!("{}: Drawing splash", function_name!());

    // Draw title
    styles
        .splash_font
        .render_aligned(
            "its3mile/london-pi-tube",
            Point::new(
                display.bounding_box().size.width as i32 / 2,
                display.bounding_box().size.height as i32 / 2,
            ),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(styles.colors.fg),
            display,
        )
        .map_err(|_| DisplayError::RenderingFailed)?;

    epd_driver
        .update_and_display_frame(spi_device, &mut display.buffer(), &mut Delay)
        .expect("Display: Failed to update display with splash");

    Ok(())
}

/// Draw and render the update to the epaper display
#[named]
fn show_update(
    display: &mut Display3in7,
    epd_driver: &mut DisplayDriver,
    spi_device: &mut DisplaySpiDevice,
    update: Update,
) -> Result<(), DisplayError> {
    let styles = DisplayStyles::new();

    info!("{}: Clearing display", function_name!());

    display
        .clear(styles.colors.bg)
        .map_err(|_| DisplayError::RenderingFailed)?;

    info!("{}: Drawing update header", function_name!());

    // Format header tightly on one or two lines
    let mut header_content = String::<128>::new();
    let _ = write!(
        &mut header_content,
        "{} Line - {}\n{}",
        update.line_name, update.station_name, update.platform_name
    );

    // Adjusted Y position to 24 to fix the top-clipping issue
    styles
        .header_font
        .render_aligned(
            header_content.as_str(),
            display.bounding_box().top_left + Point::new(10, 24),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(styles.colors.fg),
            display,
        )
        .map_err(|_| DisplayError::RenderingFailed)?;

    info!("{}: Drawing update arrivals", function_name!());

    // Pushed down to Y = 100 to comfortably clear the header block
    let mut pos = display.bounding_box().top_left + Point::new(10, 100);

    for (idx, arrival) in update.arrivals.iter().enumerate() {
        // Guard against screen overflow (leave space for footer)
        if pos.y > 245 {
            break;
        }

        // Time to station
        let mut time_to_station = String::<16>::new();
        if (arrival.time_to_station as f32 / 60.0) < 1.0 {
            let _ = write!(&mut time_to_station, "<1 min");
        } else if (arrival.time_to_station as f32 / 60.0) < 2.0 {
            let _ = write!(&mut time_to_station, "<2 mins");
        } else {
            let _ = write!(
                &mut time_to_station,
                "{} mins",
                arrival.time_to_station / 60
            );
        }

        styles
            .time_font
            .render_aligned(
                time_to_station.as_str(),
                pos,
                VerticalPosition::Baseline,
                HorizontalAlignment::Left,
                FontColor::Transparent(styles.colors.fg),
                display,
            )
            .map_err(|_| DisplayError::RenderingFailed)?;

        // Destination name
        // Offset expanded to 128px to safely clear the countdowns
        let destination_pos = pos + Point::new(128, 0);
        let destination_name = first_two_words(&arrival.destination_name);

        styles
            .bold_text_font
            .render_aligned(
                destination_name,
                destination_pos,
                VerticalPosition::Baseline,
                HorizontalAlignment::Left,
                FontColor::Transparent(styles.colors.fg),
                display,
            )
            .map_err(|_| DisplayError::RenderingFailed)?;

        // Move vertical cursor down to clear the font line
        pos.y += 24;

        // Current location, for first arrival only
        if idx == 0 {
            // Shifted to line up with destination name
            let location_pos = Point::new(140, pos.y);

            styles
                .regular_text_font
                .render_aligned(
                    arrival.current_location.as_str(),
                    location_pos,
                    VerticalPosition::Baseline,
                    HorizontalAlignment::Left,
                    FontColor::Transparent(styles.colors.fg),
                    display,
                )
                .map_err(|_| DisplayError::RenderingFailed)?;

            // Small vertical pad to clear the location line
            pos.y += 36;
        } else {
            pos.y += 12;
        }
    }

    // Bottom left, line status indicator
    // Anchor position for the footer status icon (Bottom Left)
    let icon_pos = display.bounding_box().top_left + Point::new(4, 228);

    match update.line_status.as_str() {
        "Good Service" => {
            let icon = icons::size48px::emojis::Emoji::new(BinaryColor::On);
            Image::new(&icon, icon_pos)
                .draw(&mut display.color_converted())
                .ok();
        }
        s if s.contains("Minor") || s.contains("Delay") => {
            let icon = icons::size48px::emojis::EmojiQuite::new(BinaryColor::On);
            Image::new(&icon, icon_pos)
                .draw(&mut display.color_converted())
                .ok();
        }
        s if s.contains("Severe") || s.contains("Suspended") => {
            let icon = icons::size48px::emojis::EmojiSad::new(BinaryColor::On);
            Image::new(&icon, icon_pos)
                .draw(&mut display.color_converted())
                .ok();
        }
        _ => {
            // Fallback warning triangle for anything else
            let icon = icons::size48px::emojis::EmojiPuzzled::new(BinaryColor::On);
            Image::new(&icon, icon_pos)
                .draw(&mut display.color_converted())
                .ok();
        }
    }

    // Bottom right, last updated
    if !update.last_updated_secs.is_empty() {
        let (_date, time) = split_iso8601_timestamp(update.last_updated_secs.as_str());
        let mut footer_text = String::<32>::new();
        let _ = write!(&mut footer_text, "Updated: {}", time);

        // Place at bottom right (Canvas: 480x280)
        let footer_pos = Point::new(470, 270);

        styles
            .tiny_font
            .render_aligned(
                footer_text.as_str(),
                footer_pos,
                VerticalPosition::Baseline,
                HorizontalAlignment::Right, // Anchor from right edge
                FontColor::Transparent(styles.colors.fg),
                display,
            )
            .map_err(|_| DisplayError::RenderingFailed)?;
    }

    info!("{}: Rendering update", function_name!());

    epd_driver
        .update_and_display_frame(spi_device, &mut display.buffer(), &mut Delay)
        .expect("Display: Failed to render with update data");

    Ok(())
}

/// Colors used for the display
pub struct DisplayColors {
    pub bg: Color,
    pub fg: Color,
}

// Structs to hold style and color information
pub struct DisplayStyles {
    pub colors: DisplayColors,
    pub header_font: FontRenderer,
    pub time_font: FontRenderer,
    pub bold_text_font: FontRenderer,
    pub regular_text_font: FontRenderer,
    pub splash_font: FontRenderer,
    pub tiny_font: FontRenderer,
}

impl DisplayStyles {
    pub const fn new() -> Self {
        let colors = DisplayColors {
            bg: Color::White,
            fg: Color::Black,
        };

        Self {
            colors,
            header_font: FontRenderer::new::<fonts::u8g2_font_helvB14_tf>(),
            time_font: FontRenderer::new::<fonts::u8g2_font_logisoso24_tf>(),
            bold_text_font: FontRenderer::new::<fonts::u8g2_font_logisoso24_tf>(),
            regular_text_font: FontRenderer::new::<fonts::u8g2_font_helvR14_tf>(),
            splash_font: FontRenderer::new::<fonts::u8g2_font_logisoso32_tf>(),
            tiny_font: FontRenderer::new::<fonts::u8g2_font_helvR10_tf>(),
        }
    }
}

/// Represents any error that may happen during display operations.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum DisplayError {
    /// An error occurred while rendering data
    RenderingFailed,
}

pub fn first_two_words(s: &str) -> &str {
    let mut space_count = 0;
    for (i, c) in s.char_indices() {
        if c == ' ' {
            space_count += 1;
            if space_count == 2 {
                return &s[..i];
            }
        }
    }
    s
}

pub fn split_iso8601_timestamp(s: &str) -> (&str, &str) {
    let date_end = match s.find('T') {
        Some(i) => i,
        None => s.len(),
    };
    let time_end_wo_frac = match s.find('.') {
        Some(i) => i,
        None => s.len(),
    };
    let date = &s[..date_end];
    let time = &s[date_end + 1..time_end_wo_frac];
    (date, time)
}
