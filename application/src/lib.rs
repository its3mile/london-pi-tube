//! Hardware-independent logic for london-pi-tube.
//!
//! Everything in this crate is `no_std`, free of embassy/cortex-m dependencies,
//! and therefore builds for both the RP2350 and the development host. Keep it
//! that way: anything that touches peripherals, the network stack, or
//! `embassy_time` belongs in the firmware crate.
//!
//! Run the tests on the host with:
//!
//! ```sh
//! cargo test
//! ```
#![cfg_attr(not(test), no_std)]

pub mod config;
pub mod de;
pub mod header;
pub mod json_scan;
pub mod models;
pub mod schedule;
pub mod tfl;
pub mod time;
