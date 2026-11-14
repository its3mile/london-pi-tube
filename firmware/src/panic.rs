use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};
use defmt::error;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::spi::{Blocking, Spi};
use embassy_time::Delay;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::text::Baseline;
use embedded_graphics::{
    prelude::*,
    text::{Text, TextStyleBuilder},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::color::Color;
use epd_waveshare::epd3in7::{Display3in7, EPD3in7};
use epd_waveshare::prelude::{DisplayRotation, WaveshareDisplay};
use heapless::String;
use static_cell::StaticCell;

type SpiDevice<'a> =
    ExclusiveDevice<Spi<'a, embassy_rp::peripherals::SPI1, Blocking>, Output<'a>, Delay>;
type EpdDevice<'a> = EPD3in7<SpiDevice<'a>, Input<'a>, Output<'a>, Output<'a>, Delay>;

// Static to ensure we only panic once
static PANICKING: AtomicBool = AtomicBool::new(false);

// Maximum length for panic message
const MAX_MESSAGE_LEN: usize = 200;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Make sure we only panic once
    if PANICKING.swap(true, Ordering::SeqCst) {
        loop {
            cortex_m::asm::wfe();
        }
    }

    // Create panic message
    let mut message: String<MAX_MESSAGE_LEN> = String::new();
    if let Some(location) = info.location() {
        writeln!(
            message,
            "Panic at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        )
        .ok();
    } else {
        writeln!(message, "Panic at unknown location").ok();
    }
    write!(message, "{}", info.message()).ok();

    // Re-Initialize display with the known pins (unsafe inside, but we're panicking, so what can we do ;-))
    let (epd, spidev) = init_display();
    let mut display = Display3in7::default();
    let mut delay: Delay = embassy_time::Delay;

    display.set_rotation(DisplayRotation::Rotate270);

    epd.wake_up(spidev, &mut embassy_time::Delay)
        .unwrap_or_else(|_| error!("can't wake up display for panic"));
    render_panic_message(&mut display, &message);
    epd.update_and_display_frame(spidev, &display.buffer(), &mut embassy_time::Delay)
        .unwrap_or_else(|_| error!("can't update display for panic"));

    epd.sleep(spidev, &mut delay)
        .unwrap_or_else(|_| error!("can't sleep display"));

    // Hang in an infinite loop
    loop {
        cortex_m::asm::wfe();
    }
}

/// Initialize display  for panic handler
fn init_display() -> (
    &'static mut EpdDevice<'static>,
    &'static mut SpiDevice<'static>,
) {
    // Safety: This is only called during panic, so it's safe to take ownership of these peripherals
    let p = unsafe { embassy_rp::Peripherals::steal() };

    let clk = p.PIN_10;
    let mosi = p.PIN_11;
    let cs = p.PIN_9;
    let busy = p.PIN_13;
    let dc = p.PIN_8;
    let rst = p.PIN_12;

    let busy_pin = Input::new(busy, Pull::None);
    let dc_pin = Output::new(dc, Level::Low);
    let rst_pin = Output::new(rst, Level::Low);

    let mut spi_cfg = embassy_rp::spi::Config::default();
    spi_cfg.frequency = 4_000_000;
    static SPIDEV: StaticCell<SpiDevice> = StaticCell::new();
    let spi = SPIDEV.init(
        ExclusiveDevice::new(
            embassy_rp::spi::Spi::new_blocking_txonly(p.SPI1, clk, mosi, spi_cfg),
            Output::new(cs, Level::Low),
            Delay,
        )
        .expect("Failed to initialize SPI device"),
    );

    static DELAYNS: StaticCell<Delay> = StaticCell::new();
    let delayns = DELAYNS.init(Delay {});

    static EPD: StaticCell<EpdDevice> = StaticCell::new();
    let epd = EPD.init(EPD3in7::new(spi, busy_pin, dc_pin, rst_pin, delayns, None).unwrap());

    (epd, spi)
}

/// Render panic message to display with line wrapping
fn render_panic_message(display: &mut Display3in7, message: &str) {
    let bg = Color::White;
    let fg = Color::Black;

    display
        .clear(bg)
        .unwrap_or_else(|_| error!("Failed to clear display"));

    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
        .text_color(Color::Black)
        .background_color(Color::White)
        .build();

    // Display title
    Text::with_text_style("SYSTEM PANIC", Point::new(10, 20), style, text_style)
        .draw(display)
        .ok();

    // Draw a line under the title
    use embedded_graphics::primitives::{Line, PrimitiveStyle};
    Line::new(
        Point::new(10, 45),
        Point::new(display.bounding_box().size.width as i32 - 10, 45),
    )
    .into_styled(PrimitiveStyle::with_stroke(fg, 1))
    .draw(display)
    .ok();

    // Wrap message to display width (allowing for margins)
    let max_chars_per_line = (display.bounding_box().size.width as usize - 20) / 10; // Approximate for 10px font
    let mut y = 60;

    // Each paragraph (separated by line breaks) separately
    for paragraph in message.split('\n') {
        // Skip empty paragraphs but still advance position
        if paragraph.trim().is_empty() {
            y += 25;
            continue;
        }

        // Simple line wrapping within each paragraph
        let mut current_line = String::<100>::new();
        for word in paragraph.split_whitespace() {
            if current_line.len() + word.len() + 1 > max_chars_per_line {
                // Draw current line and start a new one
                Text::with_text_style(&current_line, Point::new(10, y), style, text_style)
                    .draw(display)
                    .ok();

                y += 25; // Line height
                current_line.clear();
                let _ = write!(current_line, "{}", word);
            } else {
                if !current_line.is_empty() {
                    let _ = write!(current_line, " ");
                }
                let _ = write!(current_line, "{}", word);
            }
        }

        // Draw the last line of this paragraph
        if !current_line.is_empty() {
            Text::with_text_style(&current_line, Point::new(10, y), style, text_style)
                .draw(display)
                .ok();
            y += 25; // Move to the next line after each paragraph
        }
    }
}
