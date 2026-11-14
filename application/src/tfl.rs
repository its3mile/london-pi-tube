//! Parsing of TfL API response bodies.
//!
//! These functions take the raw HTTP body so that they can be tested against
//! captured responses (see `tests/fixtures`) without any networking.
//!
//! Note: strings are not unescaped. JSON escapes such as `&` are kept
//! verbatim. Unescaping would need `serde_json_core::from_slice_escaped`, which
//! fails on *any* escaped string longer than its buffer, including skipped
//! fields such as long disruption descriptions.

use core::fmt;

use heapless::Vec;
use serde::de::{DeserializeSeed, SeqAccess, Visitor};

use crate::de::FirstElement;
use crate::models::{Line, Prediction};

pub use serde_json_core::de::Error;

/// Parse a `/StopPoint/{id}/Arrivals` body.
///
/// Keeps only predictions whose `platformName` contains `platform`, and of
/// those only the `N` arriving soonest, sorted by `time_to_station`.
///
/// The response lists every arrival at the station across all platforms and
/// lines, often 10–30 entries at busy times. Elements are filtered as they are
/// parsed, so the response length never causes an error.
///
/// Returns an empty `Vec` if the body is a valid but empty array, or nothing
/// matched `platform`.
pub fn parse_predictions<const N: usize>(
    body: &[u8],
    platform: &str,
) -> Result<Vec<Prediction, N>, Error> {
    let mut de = serde_json_core::de::Deserializer::new(body, None);
    let predictions = PlatformFilter::<N> { platform }.deserialize(&mut de)?;
    de.end()?;
    Ok(predictions)
}

/// Parse a `/Line/{id}/Status` body.
///
/// - `Ok(None)`: the response was an empty array (e.g. an unknown line id).
/// - `Ok(Some(line))` with `line.line_status == None`: the line exists but no
///   status was reported.
///
/// Only the first line in the response is used.
pub fn parse_line_status(body: &[u8]) -> Result<Option<Line>, Error> {
    let mut de = serde_json_core::de::Deserializer::new(body, None);
    let line = FirstElement::<Line>::new().deserialize(&mut de)?;
    de.end()?;
    Ok(line)
}

struct PlatformFilter<'a, const N: usize> {
    platform: &'a str,
}

impl<'de, const N: usize> DeserializeSeed<'de> for PlatformFilter<'_, N> {
    type Value = Vec<Prediction, N>;

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de, const N: usize> Visitor<'de> for PlatformFilter<'_, N> {
    type Value = Vec<Prediction, N>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an array of predictions")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut kept: Vec<Prediction, N> = Vec::new();
        while let Some(p) = seq.next_element::<Prediction>()? {
            if p.platform_name.contains(self.platform) {
                keep_soonest(&mut kept, p);
            }
        }
        kept.sort_unstable_by_key(|p| p.time_to_station);
        Ok(kept)
    }
}

/// Insert `p`, or if full, replace the latest-arriving prediction when `p` is sooner.
fn keep_soonest<const N: usize>(kept: &mut Vec<Prediction, N>, p: Prediction) {
    let Err(p) = kept.push(p) else { return };
    let latest = kept
        .iter()
        .enumerate()
        .max_by_key(|(_, q)| q.time_to_station)
        .map(|(i, q)| (i, q.time_to_station));
    if let Some((i, latest_time)) = latest {
        if p.time_to_station < latest_time {
            kept[i] = p;
        }
    }
}
