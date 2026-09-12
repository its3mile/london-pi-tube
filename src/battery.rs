//! Battery monitoring utilities for RP2350 / Pico 2 W.

/// Reference voltage for the RP2350 ADC (3.3V).
const ADC_VREF: f32 = 3.3;

/// Maximum integer value produced by a 12-bit ADC (2^12).
const ADC_MAX_COUNT: f32 = 4096.0;

/// Onboard resistor divider step-down ratio (3:1).
/// This is a 150k 75k ohm divider for the pico plus 2w according to Vsys sens voltage divider diagram on
/// sheet 2 of https://cdn.shopify.com/s/files/1/0174/1800/files/Pimoroni_Pico_Plus_2_W_Schematic.pdf?v=1727350279
const HARDWARE_DIVIDER_RATIO: f32 = 3.0;

/// Precalculated factor to convert a 12-bit ADC count directly to VSYS voltage.
///
/// Formula: `(ADC_VREF / ADC_MAX_COUNT) * HARDWARE_DIVIDER_RATIO`
const VOLTAGE_CONVERSION_FACTOR: f32 = (ADC_VREF / ADC_MAX_COUNT) * HARDWARE_DIVIDER_RATIO;

/// Nominal fully charged single-cell LiPo voltage.
const FULL_BATTERY_VOLTAGE: f32 = 4.2;

/// Standard safety cutoff voltage for an empty LiPo battery (prevents cell damage).
const EMPTY_BATTERY_VOLTAGE: f32 = 3.0;

/// Converts a raw 12-bit ADC sample (0–4095) to the actual system voltage (VSYS).
///
/// Account for both the 0.0V–3.3V ADC input range and the onboard 3:1 voltage divider.
pub fn calculate_voltage(adc: u16) -> f32 {
    adc as f32 * VOLTAGE_CONVERSION_FACTOR
}

/// Calculates remaining battery charge as a percentage bounded between 0.0% and 100.0%.
///
/// Uses linear interpolation between `EMPTY_BATTERY_VOLTAGE` and `FULL_BATTERY_VOLTAGE`.
pub fn calculate_percentage(voltage: f32) -> f32 {
    let percentage =
        100.0 * (voltage - EMPTY_BATTERY_VOLTAGE) / (FULL_BATTERY_VOLTAGE - EMPTY_BATTERY_VOLTAGE);
    percentage.clamp(0.0, 100.0)
}
