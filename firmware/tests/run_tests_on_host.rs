//! Placeholder so that `cargo test` from the workspace root explains itself
//! instead of failing with "can't find crate for `std`".
//!
//! The workspace's default target is the RP2350 (thumbv8m), which has no
//! standard library and so cannot run a test harness. The tests live in the
//! hardware-independent `application` crate and run on the development host.
#![no_std]
#![no_main]

compile_error!(
    "the firmware has no host tests; the tests live in the `application` crate.
       Run them with `cargo test-host`, or `cargo test` from the application/ directory."
);

// Only here so rustc reports the message above and nothing else.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
