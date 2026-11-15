//! Streaming extraction of one top-level string field from a JSON object.
//!
//! Some TfL responses are far larger than the firmware's HTTP buffer (a
//! `StopPoint` for a busy interchange runs to hundreds of KB) when all that is
//! needed is one field near the start. [`TopLevelStringScanner`] is fed the
//! body in chunks of any size, as it arrives, and reports the field's value as
//! soon as it has been read, so the rest of the body never needs to be held.
//!
//! Only the outermost object is searched: a key of the same name inside a
//! nested object or array (e.g. a child stop's `commonName`) is ignored.

use heapless::Vec;

/// Finds the string value of `key` in the top-level JSON object.
///
/// The value is unescaped, and truncated to at most `N` bytes on a character
/// boundary.
pub struct TopLevelStringScanner<const N: usize> {
    key: &'static str,
    state: State,
    /// Nesting depth of objects and arrays: the top-level object is depth 1.
    depth: u32,
    /// At depth 1, whether the next string is a key (after `{` or `,`) rather
    /// than a value (after `:`).
    expect_key: bool,
    /// The key currently being read, if it could still match.
    key_buf: Vec<u8, 64>,
    key_too_long: bool,
    value: Vec<u8, N>,
    /// Pending `\uXXXX` escape in the value: hex digits still to read and the
    /// code point so far.
    unicode: Option<(u8, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Outside any string.
    Structure,
    /// Inside a string that is neither the key we want nor its value.
    OtherString { escaped: bool },
    /// Inside a top-level key.
    Key { escaped: bool },
    /// After the wanted key, waiting for `:`.
    AfterKey,
    /// After `"key":`, waiting for the value to start.
    AfterColon,
    /// Inside the wanted value.
    Value { escaped: bool },
    /// The value has been read.
    Done,
}

impl<const N: usize> TopLevelStringScanner<N> {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            state: State::Structure,
            depth: 0,
            expect_key: false,
            key_buf: Vec::new(),
            key_too_long: false,
            value: Vec::new(),
            unicode: None,
        }
    }

    /// Feed the next chunk of the body. Returns the value once it has been
    /// read (and on every later call); `None` while still searching.
    pub fn feed(&mut self, chunk: &[u8]) -> Option<&str> {
        for &byte in chunk {
            if self.state == State::Done {
                break;
            }
            self.step(byte);
        }
        self.value()
    }

    /// The value, if it has been read.
    pub fn value(&self) -> Option<&str> {
        if self.state != State::Done {
            return None;
        }
        // Truncation may have split a multi-byte character: drop the partial one.
        Some(match core::str::from_utf8(&self.value) {
            Ok(s) => s,
            Err(e) => core::str::from_utf8(&self.value[..e.valid_up_to()]).unwrap_or_default(),
        })
    }

    fn step(&mut self, byte: u8) {
        match self.state {
            State::Structure => self.structure(byte),
            State::OtherString { escaped } => {
                self.state = match (escaped, byte) {
                    (true, _) => State::OtherString { escaped: false },
                    (false, b'\\') => State::OtherString { escaped: true },
                    (false, b'"') => State::Structure,
                    (false, _) => State::OtherString { escaped: false },
                };
            }
            State::Key { escaped } => match (escaped, byte) {
                (true, _) => {
                    // Escapes never occur in TfL's keys; treat as a non-match.
                    self.key_too_long = true;
                    self.state = State::Key { escaped: false };
                }
                (false, b'\\') => self.state = State::Key { escaped: true },
                (false, b'"') => {
                    let matched = !self.key_too_long && self.key_buf == self.key.as_bytes();
                    self.state = if matched {
                        State::AfterKey
                    } else {
                        State::Structure
                    };
                }
                (false, _) => {
                    if self.key_buf.push(byte).is_err() {
                        self.key_too_long = true;
                    }
                }
            },
            State::AfterKey => match byte {
                b' ' | b'\t' | b'\r' | b'\n' => {}
                b':' => {
                    self.expect_key = false;
                    self.state = State::AfterColon;
                }
                // Not valid JSON; resume scanning.
                _ => self.reprocess(byte),
            },
            State::AfterColon => match byte {
                b' ' | b'\t' | b'\r' | b'\n' => {}
                b'"' => self.state = State::Value { escaped: false },
                // The key exists but its value is not a string (e.g. null):
                // keep looking, though a well-formed object has no second one.
                _ => self.reprocess(byte),
            },
            State::Value { escaped } => self.value_byte(escaped, byte),
            State::Done => {}
        }
    }

    /// Handle `byte` as structure, after leaving a state that did not consume it.
    fn reprocess(&mut self, byte: u8) {
        self.state = State::Structure;
        self.structure(byte);
    }

    fn structure(&mut self, byte: u8) {
        match byte {
            b'{' | b'[' => {
                self.depth += 1;
                self.expect_key = self.depth == 1 && byte == b'{';
            }
            b'}' | b']' => {
                self.depth = self.depth.saturating_sub(1);
                self.expect_key = false;
            }
            b',' if self.depth == 1 => self.expect_key = true,
            b':' if self.depth == 1 => self.expect_key = false,
            b'"' if self.depth == 1 && self.expect_key => {
                self.key_buf.clear();
                self.key_too_long = false;
                self.state = State::Key { escaped: false };
            }
            b'"' => self.state = State::OtherString { escaped: false },
            _ => {}
        }
    }

    fn value_byte(&mut self, escaped: bool, byte: u8) {
        if let Some((remaining, code)) = self.unicode {
            let digit = (byte as char).to_digit(16).unwrap_or(0);
            let code = code << 4 | digit;
            if remaining > 1 {
                self.unicode = Some((remaining - 1, code));
            } else {
                self.unicode = None;
                self.push_char(char::from_u32(code).unwrap_or('\u{FFFD}'));
            }
            return;
        }
        if escaped {
            self.state = State::Value { escaped: false };
            match byte {
                b'u' => self.unicode = Some((4, 0)),
                b'n' | b'r' | b't' => self.push_char(' '),
                b'b' | b'f' => {}
                // `\"`, `\\` and `\/` are the character itself.
                _ => self.push_byte(byte),
            }
            return;
        }
        match byte {
            b'\\' => self.state = State::Value { escaped: true },
            b'"' => self.state = State::Done,
            _ => self.push_byte(byte),
        }
    }

    fn push_char(&mut self, c: char) {
        let mut buf = [0; 4];
        for &b in c.encode_utf8(&mut buf).as_bytes() {
            self.push_byte(b);
        }
    }

    /// Append to the value, silently dropping bytes beyond `N`.
    fn push_byte(&mut self, byte: u8) {
        let _ = self.value.push(byte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan<const N: usize>(
        json: &str,
        key: &'static str,
        chunk: usize,
    ) -> Option<std::string::String> {
        let mut scanner = TopLevelStringScanner::<N>::new(key);
        for part in json.as_bytes().chunks(chunk) {
            if let Some(v) = scanner.feed(part) {
                return Some(v.to_owned());
            }
        }
        None
    }

    /// Every chunk size from 1 byte up, so boundaries land everywhere.
    fn scan_all_chunkings(json: &str, key: &'static str) -> Option<std::string::String> {
        let first = scan::<72>(json, key, json.len().max(1));
        for chunk in 1..=json.len().min(40) {
            assert_eq!(scan::<72>(json, key, chunk), first, "chunk size {chunk}");
        }
        first
    }

    #[test]
    fn finds_top_level_field() {
        let json =
            r#"{"id":"940GZZLUEPY","commonName":"East Putney Underground Station","lat":51.4}"#;
        assert_eq!(
            scan_all_chunkings(json, "commonName").as_deref(),
            Some("East Putney Underground Station")
        );
    }

    #[test]
    fn ignores_nested_fields_with_the_same_key() {
        let json = r#"{"lines":[{"commonName":"wrong"}],"child":{"commonName":"also wrong"},"commonName":"right"}"#;
        assert_eq!(
            scan_all_chunkings(json, "commonName").as_deref(),
            Some("right")
        );
    }

    #[test]
    fn ignores_the_key_appearing_as_a_value() {
        let json = r#"{"note":"commonName","x":["commonName"],"commonName":"right"}"#;
        assert_eq!(
            scan_all_chunkings(json, "commonName").as_deref(),
            Some("right")
        );
    }

    #[test]
    fn handles_whitespace() {
        let json = "{\n  \"id\" : 1 ,\n  \"commonName\"  :\n  \"Bank\"\n}";
        assert_eq!(
            scan_all_chunkings(json, "commonName").as_deref(),
            Some("Bank")
        );
    }

    #[test]
    fn unescapes_the_value() {
        let json = r#"{"commonName":"King's \"Cross\" \\ St/Pancras"}"#;
        assert_eq!(
            scan_all_chunkings(json, "commonName").as_deref(),
            Some(r#"King's "Cross" \ St/Pancras"#)
        );
    }

    #[test]
    fn escaped_quotes_in_other_strings_do_not_confuse_it() {
        let json = r#"{"a":"x\",\"commonName\":\"wrong","commonName":"right"}"#;
        assert_eq!(
            scan_all_chunkings(json, "commonName").as_deref(),
            Some("right")
        );
    }

    #[test]
    fn truncates_on_a_char_boundary() {
        let json = r#"{"commonName":"Café Underground"}"#;
        // "Caf" is 3 bytes, "é" is 2: a 4-byte limit must not split it.
        assert_eq!(scan::<4>(json, "commonName", 1).as_deref(), Some("Caf"));
        assert_eq!(scan::<5>(json, "commonName", 1).as_deref(), Some("Café"));
    }

    #[test]
    fn missing_or_non_string_field_is_none() {
        assert_eq!(scan_all_chunkings(r#"{"id":"x"}"#, "commonName"), None);
        assert_eq!(
            scan_all_chunkings(r#"{"commonName":null}"#, "commonName"),
            None
        );
        assert_eq!(
            scan_all_chunkings(r#"[{"commonName":"in an array"}]"#, "commonName"),
            None
        );
        assert_eq!(scan_all_chunkings("", "commonName"), None);
    }

    #[test]
    fn value_cut_off_mid_string_is_none() {
        assert_eq!(
            scan_all_chunkings(r#"{"commonName":"East Put"#, "commonName"),
            None
        );
    }

    #[test]
    fn stops_consuming_once_found() {
        let mut scanner = TopLevelStringScanner::<72>::new("commonName");
        assert_eq!(scanner.feed(br#"{"commonName":"A"}"#), Some("A"));
        // Later input, even malformed, does not change the result.
        assert_eq!(scanner.feed(br#"garbage "commonName":"B""#), Some("A"));
    }
}
