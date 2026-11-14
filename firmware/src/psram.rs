//! External PSRAM: 8 MiB APS6404L on QMI chip select 1 (GPIO 47) of the
//! Pimoroni Pico Plus 2 W.
//!
//! Once initialised, the PSRAM is memory-mapped at 0x1100_0000 through the XIP
//! cache and behaves like ordinary (slower) RAM. The linker script provides a
//! NOLOAD `.psram` section there.
//!
//! Allocate from it with [`psram_static!`], which:
//! - require the [`Psram`] handle, so nothing is written before [`init`] runs;
//! - hand out each allocation once, as `&'static mut`, like `static_cell`.
//!
//! Do not point DMA at PSRAM allocations without handling the XIP cache.

use embassy_rp::Peri;
use embassy_rp::peripherals::{PIN_47, QMI_CS1};
use embassy_rp::psram::{Config, Error, Psram};
use embassy_rp::qmi_cs1::QmiCs1;

/// Detect and configure the PSRAM for memory-mapped access.
///
/// Call after `embassy_rp::init`, so the system clock is final: the QMI timing
/// is derived from it.
pub fn init(
    qmi_cs1: Peri<'static, QMI_CS1>,
    cs_pin: Peri<'static, PIN_47>,
) -> Result<Psram<'static>, Error> {
    let mut config = Config::aps6404l();
    // The driver's default assumes a 125 MHz system clock; the RP2350 runs at 150 MHz.
    config.clock_hz = embassy_rp::clocks::clk_sys_freq();
    Psram::new(QmiCs1::new(qmi_cs1, cs_pin), config)
}

/// Place `$init` in PSRAM and return `&'static mut $ty`.
///
/// `$init` is built on the stack and then moved, so keep it to types that fit
/// comfortably there.
///
/// Panics if the same invocation runs twice.
///
/// ```ignore
/// let display: &'static mut Display3in7 = psram_static!(&psram, Display3in7, Display3in7::default());
/// ```
#[macro_export]
macro_rules! psram_static {
    ($psram:expr, $ty:ty, $init:expr) => {{
        let slot = $crate::psram_slot!($psram, $ty);
        slot.write($init)
    }};
}

/// Implementation detail of [`psram_static!`]: a unique uninitialised `.psram`
/// static per expansion, handed out once.
#[doc(hidden)]
#[macro_export]
macro_rules! psram_slot {
    ($psram:expr, $ty:ty) => {{
        // Only callable with the handle returned by `psram::init`, so the PSRAM is ready.
        let _: &::embassy_rp::psram::Psram<'_> = $psram;

        #[unsafe(link_section = ".psram")]
        static mut SLOT: ::core::mem::MaybeUninit<$ty> = ::core::mem::MaybeUninit::uninit();
        // In SRAM, so zeroed at boot unlike the `.psram` section.
        static TAKEN: ::core::sync::atomic::AtomicBool =
            ::core::sync::atomic::AtomicBool::new(false);

        ::core::assert!(
            !TAKEN.swap(true, ::core::sync::atomic::Ordering::AcqRel),
            "PSRAM allocation taken twice"
        );
        // SAFETY: `TAKEN` guarantees this is the only reference ever created to `SLOT`.
        unsafe { &mut *(&raw mut SLOT) }
    }};
}
