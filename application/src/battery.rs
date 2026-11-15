//! Battery state from the VSYS sense pin.
//!
//! What VSYS means depends on how the board is powered:
//!
//! - **On battery** (LiPo SHIM, USB unplugged): VSYS is the cell voltage,
//!   roughly 3.3 V (empty) to 4.2 V (full).
//! - **On USB**: VSYS is VBUS minus a diode drop, roughly 4.6–5.0 V, whatever
//!   the state of the battery. The charge level cannot be measured in this
//!   state; the battery is charging (or absent).
//!
//! A single LiPo cell never exceeds 4.2 V, so a VSYS above
//! [`EXTERNAL_POWER_THRESHOLD_MV`] means external power.
//!
//! # Why a calibration rather than a divider ratio
//!
//! The Pico SDK lists GPIO 43 as the VSYS sense pin of the Pimoroni Pico Plus
//! 2 W, and community sources describe a 150k/75k (3:1) divider. Measured on
//! the board, the pin is fed from VSYS through a high resistance but has no
//! pull to ground: left alone it floats to about 3.1 V whatever VSYS is.
//! Enabling the RP2350's internal pull-down supplies the missing leg, and the
//! pin then tracks VSYS (see issue #12 for the measurements).
//!
//! The resulting ratio depends on the pull-down, which is not a precision
//! part, so raw readings are converted with a [`Calibration`] measured on the
//! device rather than a calculated ratio.
//!
//! All arithmetic is integer millivolts: no floating point needed.

/// VSYS above this cannot be a LiPo cell, so the board is on external power.
///
/// Sits between a full cell (4.2 V, plus a few percent of measurement error)
/// and the lowest VSYS expected on USB (about 4.4 V with a weak supply).
pub const EXTERNAL_POWER_THRESHOLD_MV: u32 = 4350;

/// Converts raw ADC readings of the VSYS sense pin to VSYS millivolts, as the
/// straight line through two measured points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calibration {
    /// (raw ADC reading, VSYS in millivolts measured at the same time).
    low: (u16, u32),
    high: (u16, u32),
}

impl Calibration {
    /// A calibration from two measurements at different voltages, e.g. one on
    /// battery and one on USB, each paired with a multimeter reading of VSYS.
    ///
    /// Panics (at compile time, when used in a `const`) if the two raw
    /// readings are equal.
    pub const fn two_point(a: (u16, u32), b: (u16, u32)) -> Self {
        assert!(
            a.0 != b.0,
            "calibration points must have different raw readings"
        );
        if a.0 < b.0 {
            Self { low: a, high: b }
        } else {
            Self { low: b, high: a }
        }
    }

    /// A calibration from a single measurement, assuming the reading is
    /// proportional to VSYS (a raw reading of 0 means 0 V).
    pub const fn one_point(raw: u16, millivolts: u32) -> Self {
        Self::two_point((0, 0), (raw, millivolts))
    }

    /// Convert a raw ADC reading to VSYS millivolts. Readings outside the
    /// calibrated range are extrapolated along the same line; the result never
    /// goes below zero.
    pub fn millivolts(&self, raw: u16) -> u32 {
        let (raw_lo, mv_lo) = (self.low.0 as i64, self.low.1 as i64);
        let (raw_hi, mv_hi) = (self.high.0 as i64, self.high.1 as i64);
        let mv = mv_lo + (raw as i64 - raw_lo) * (mv_hi - mv_lo) / (raw_hi - raw_lo);
        mv.max(0) as u32
    }
}

/// How the board is being powered, and the battery charge if it can be known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerState {
    /// Running from the battery, with the estimated charge remaining (0–100).
    Battery { percent: u8 },
    /// Running from USB (the battery, if fitted, is charging). The charge
    /// level is unknown.
    External,
}

/// One battery measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryReading {
    /// VSYS, in millivolts.
    pub millivolts: u32,
    pub state: PowerState,
}

impl BatteryReading {
    /// Interpret a raw ADC reading (ideally an average of several) of the
    /// VSYS sense pin.
    pub fn from_adc(raw: u16, calibration: &Calibration) -> Self {
        let millivolts = calibration.millivolts(raw);
        let state = if millivolts > EXTERNAL_POWER_THRESHOLD_MV {
            PowerState::External
        } else {
            PowerState::Battery {
                percent: charge_percent(millivolts),
            }
        };
        Self { millivolts, state }
    }
}

/// Discharge curve of a single LiPo cell: (millivolts, percent remaining),
/// highest first.
///
/// A LiPo's voltage is far from linear in its charge: it spends most of its
/// capacity between 3.7 V and 3.9 V and then falls away quickly, so a straight
/// line between "empty" and "full" reads badly wrong in the middle. These are
/// typical resting voltages; under load (Wi-Fi) the cell reads slightly low.
const DISCHARGE_CURVE: [(u32, u8); 12] = [
    (4200, 100),
    (4110, 90),
    (4020, 80),
    (3950, 70),
    (3870, 60),
    (3840, 50),
    (3800, 40),
    (3770, 30),
    (3730, 20),
    (3690, 10),
    (3610, 5),
    (3300, 0),
];

/// Estimate the remaining charge (0–100) of a single LiPo cell from its
/// voltage, interpolating linearly between points of the discharge curve.
pub fn charge_percent(millivolts: u32) -> u8 {
    let (top_mv, top_pct) = DISCHARGE_CURVE[0];
    if millivolts >= top_mv {
        return top_pct;
    }
    for pair in DISCHARGE_CURVE.windows(2) {
        let (hi_mv, hi_pct) = pair[0];
        let (lo_mv, lo_pct) = pair[1];
        if millivolts >= lo_mv {
            let span_mv = hi_mv - lo_mv;
            let span_pct = (hi_pct - lo_pct) as u32;
            return lo_pct + ((millivolts - lo_mv) * span_pct / span_mv) as u8;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Illustrative values: the raw readings were seen on the device (613 on
    /// battery, 815 on USB), but VSYS was not measured at the time.
    const CAL: Calibration = Calibration::two_point((613, 3640), (815, 4840));

    #[test]
    fn calibration_reproduces_its_points() {
        assert_eq!(CAL.millivolts(613), 3640);
        assert_eq!(CAL.millivolts(815), 4840);
    }

    #[test]
    fn calibration_interpolates_and_extrapolates() {
        // Halfway between the two points.
        assert_eq!(CAL.millivolts(714), 4240);
        // Below and above the calibrated range, on the same line.
        assert_eq!(CAL.millivolts(512), 3040);
        assert_eq!(CAL.millivolts(916), 5440);
    }

    #[test]
    fn calibration_point_order_does_not_matter() {
        let swapped = Calibration::two_point((815, 4840), (613, 3640));
        assert_eq!(swapped, CAL);
    }

    #[test]
    fn calibration_never_goes_negative() {
        assert_eq!(CAL.millivolts(0), 0);
    }

    #[test]
    fn one_point_calibration_is_proportional() {
        let cal = Calibration::one_point(815, 4840);
        assert_eq!(cal.millivolts(0), 0);
        assert_eq!(cal.millivolts(815), 4840);
        assert_eq!(cal.millivolts(1630), 9680);
        assert!(cal.millivolts(613).abs_diff(3640) < 5);
    }

    #[test]
    fn curve_endpoints_and_clamping() {
        assert_eq!(charge_percent(4200), 100);
        assert_eq!(charge_percent(4300), 100);
        assert_eq!(charge_percent(3300), 0);
        assert_eq!(charge_percent(3000), 0);
        assert_eq!(charge_percent(0), 0);
    }

    #[test]
    fn curve_points_are_exact() {
        for (mv, pct) in DISCHARGE_CURVE {
            assert_eq!(charge_percent(mv), pct, "{mv} mV");
        }
    }

    #[test]
    fn curve_interpolates_between_points() {
        // Halfway between 3840 mV (50%) and 3870 mV (60%).
        assert_eq!(charge_percent(3855), 55);
        // Halfway between 4110 mV (90%) and 4200 mV (100%).
        assert_eq!(charge_percent(4155), 95);
    }

    #[test]
    fn curve_is_monotonic() {
        let mut previous = 0;
        for mv in 3000..=4300 {
            let pct = charge_percent(mv);
            assert!(pct >= previous, "{mv} mV: {pct}% < {previous}%");
            assert!(pct <= 100);
            previous = pct;
        }
    }

    /// The nominal 3.7 V of a LiPo is well below half charge, which is what a
    /// straight line from 3.0 V to 4.2 V gets wrong (it would say 58%).
    #[test]
    fn nominal_voltage_is_not_half() {
        assert!(charge_percent(3700) < 20);
    }

    #[test]
    fn on_battery() {
        let reading = BatteryReading::from_adc(613, &CAL);
        assert_eq!(reading.millivolts, 3640);
        assert!(matches!(
            reading.state,
            PowerState::Battery { percent: 5..=8 }
        ));
    }

    #[test]
    fn on_usb() {
        let reading = BatteryReading::from_adc(815, &CAL);
        assert_eq!(reading.state, PowerState::External);
    }

    #[test]
    fn threshold_separates_full_cell_from_usb() {
        let cal = Calibration::one_point(1000, 5000);
        // A full cell, read 2% high, is still a battery.
        assert_eq!(
            BatteryReading::from_adc(857, &cal).state, // 4285 mV
            PowerState::Battery { percent: 100 }
        );
        // The lowest VSYS expected on USB is still external power.
        assert_eq!(
            BatteryReading::from_adc(880, &cal).state, // 4400 mV
            PowerState::External
        );
    }

    /// A disconnected sense pin reads near zero: report it as an empty
    /// battery rather than panicking or wrapping.
    #[test]
    fn near_zero_reading() {
        let reading = BatteryReading::from_adc(3, &CAL);
        assert_eq!(reading.state, PowerState::Battery { percent: 0 });
    }
}
