//! Display task

use ::function_name::named;
use defmt::*;

use embedded_graphics::{prelude::*, primitives::PrimitiveStyle};
use epd_waveshare::color::Color;
use epd_waveshare::epd3in7::Display3in7;
use epd_waveshare::prelude::WaveshareDisplay;
use u8g2_fonts::{
    FontRenderer, fonts,
    types::{FontColor, HorizontalAlignment, VerticalPosition},
};

use embassy_rp::gpio::{Input, Output};
use embassy_rp::spi;
use embassy_rp::spi::Spi;

use embassy_time::Delay;

use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::{epd3in7::*, prelude::*};

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
        info!("{}: Wait for signal...", function_name!());
        NOTIFY.wait().await;
        info!(
            "{}: Signal received! Preparing full screen refresh...",
            function_name!()
        );

        // Acquire lock to read data update
        let _current_data = {
            let update = UPDATE.lock().await;
            (*update).clone()
        }; // Release lock

        let _ = display
            .clear(styles.colors.bg)
            .map_err(|_| DisplayError::RenderingFailed);

        info!("{}: Drawing update", function_name!());

        let _ = styles
            .title_font
            .render_aligned(
                "Hello World! I'm not implemented yet...",
                Point::new(
                    display.bounding_box().size.width as i32 / 2,
                    display.bounding_box().size.height as i32 / 2,
                ),
                VerticalPosition::Baseline,
                HorizontalAlignment::Center,
                FontColor::Transparent(styles.colors.fg),
                &mut display,
            )
            .map_err(|_| DisplayError::RenderingFailed);

        info!("{}: Rendering update", function_name!());

        epd_driver
            .update_and_display_frame(&mut spi_device, &mut display.buffer(), &mut Delay)
            .expect("Display: Failed to update display with update");

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
    info!("{}: Clearning display", function_name!());

    let styles = DisplayStyles::new();

    display
        .clear(styles.colors.bg)
        .map_err(|_| DisplayError::RenderingFailed)?;

    info!("{}: Drawing splash", function_name!());

    // Draw title
    styles
        .title_font
        .render_aligned(
            "its3mile/london-pi-tube",
            Point::new(
                display.bounding_box().size.width as i32 / 2,
                display.bounding_box().size.height as i32 / 2,
            ),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(styles.colors.fg),
            display,
        )
        .map_err(|_| DisplayError::RenderingFailed)?;

    info!("{}: Rendering splash", function_name!());
    epd_driver
        .update_and_display_frame(spi_device, &mut display.buffer(), &mut Delay)
        .expect("Display: Failed to update display with splash");

    Ok(())
}

// Structs to hold style and color information
#[allow(dead_code)]
pub struct DisplayStyles {
    status_bar: PrimitiveStyle<Color>,
    value_font: FontRenderer,
    unit_font: FontRenderer,
    footnote_font: FontRenderer,
    title_font: FontRenderer,
    colors: DisplayColors,
}

/// Colors used for the display (2 colors for now only)
pub struct DisplayColors {
    pub bg: Color,
    pub fg: Color,
}

impl DisplayStyles {
    pub const fn new() -> Self {
        let colors = DisplayColors {
            bg: Color::White,
            fg: Color::Black,
        };

        Self {
            status_bar: PrimitiveStyle::with_fill(colors.fg),
            value_font: FontRenderer::new::<fonts::u8g2_font_fub30_tf>(),
            unit_font: FontRenderer::new::<fonts::u8g2_font_fub17_tf>(),
            footnote_font: FontRenderer::new::<fonts::u8g2_font_fub11_tf>(),
            title_font: FontRenderer::new::<fonts::u8g2_font_fub25_tf>(),
            colors,
        }
    }
}

/// Represents any error that may happen during display operations.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum DisplayError {
    /// An error occurred while rendering data
    RenderingFailed,
}
