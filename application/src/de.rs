//! Serde helpers that keep TfL API parsing from failing on unexpected but
//! harmless data.
//!
//! `heapless` collections reject over-capacity input by failing the *whole*
//! deserialisation, so a single long string or an extra array element anywhere
//! in the response would otherwise discard every prediction in it.

use core::fmt;
use core::marker::PhantomData;

use heapless::String;
use serde::Deserializer;
use serde::de::{self, DeserializeSeed, IgnoredAny, SeqAccess, Visitor};

/// Copy `s` into a `String<N>`, truncating at the last char boundary that fits.
pub fn truncate<const N: usize>(s: &str) -> String<N> {
    let mut end = s.len().min(N);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::new();
    // Cannot fail: `end <= N`.
    let _ = out.push_str(&s[..end]);
    out
}

/// `deserialize_with` target for `heapless::String<N>` fields.
///
/// Accepts a string (truncated to `N` bytes) or `null` (empty string). Combine
/// with `#[serde(default)]` to also tolerate the field being absent.
pub fn truncating_string<'de, D, const N: usize>(deserializer: D) -> Result<String<N>, D::Error>
where
    D: Deserializer<'de>,
{
    struct TruncatingVisitor<const N: usize>;

    impl<'de, const N: usize> Visitor<'de> for TruncatingVisitor<N> {
        type Value = String<N>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a string or null")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(truncate(v))
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(String::new())
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(String::new())
        }

        fn visit_some<D2: Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            d.deserialize_str(self)
        }
    }

    deserializer.deserialize_option(TruncatingVisitor::<N>)
}

/// Deserialises a JSON array, keeping only its first element and skipping the
/// rest without allocating. Yields `None` for an empty array.
pub struct FirstElement<T>(PhantomData<T>);

impl<T> FirstElement<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for FirstElement<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'de, T: de::Deserialize<'de>> Visitor<'de> for FirstElement<T> {
    type Value = Option<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let first = seq.next_element::<T>()?;
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(first)
    }
}

impl<'de, T: de::Deserialize<'de>> DeserializeSeed<'de> for FirstElement<T> {
    type Value = Option<T>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

/// `deserialize_with` target for an `Option<T>` field fed by a JSON array.
pub fn first_element<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: de::Deserialize<'de>,
{
    FirstElement::<T>::new().deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_fits() {
        assert_eq!(truncate::<8>("abc").as_str(), "abc");
        assert_eq!(truncate::<3>("abc").as_str(), "abc");
        assert_eq!(truncate::<0>("abc").as_str(), "");
    }

    #[test]
    fn truncate_cuts_long_strings() {
        assert_eq!(truncate::<5>("Earl's Court").as_str(), "Earl'");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        // 'é' is two bytes; cutting at byte 2 would split it.
        assert_eq!(truncate::<2>("aé").as_str(), "a");
        assert_eq!(truncate::<3>("aé").as_str(), "aé");
    }
}
