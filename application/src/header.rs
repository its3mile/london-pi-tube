//! The display header: line, station and platform, any of which may be unknown.
//!
//! ```text
//! District Line - East Putney Underground Station
//! Eastbound - Platform 1
//! ```

use core::fmt::{self, Write};

/// Write the header text: `"<line> - <station>"`, then the platform on a
/// second line. Parts that are empty are left out along with their separators,
/// so a partial header still reads naturally.
pub fn write_header(
    out: &mut impl Write,
    line: &str,
    station: &str,
    platform: &str,
) -> fmt::Result {
    match (line.is_empty(), station.is_empty()) {
        (false, false) => {
            write_line_name(out, line)?;
            write!(out, " - {station}")?;
        }
        (false, true) => write_line_name(out, line)?,
        (true, false) => out.write_str(station)?,
        (true, true) => {}
    }
    if !platform.is_empty() {
        if !line.is_empty() || !station.is_empty() {
            out.write_char('\n')?;
        }
        out.write_str(platform)?;
    }
    Ok(())
}

/// `"District"` becomes `"District Line"`; names that already say so
/// (`"Elizabeth line"`) or are not called lines (`"DLR"`) are left alone.
fn write_line_name(out: &mut impl Write, name: &str) -> fmt::Result {
    let already_a_line = name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case("line");
    if already_a_line || name == "DLR" {
        out.write_str(name)
    } else {
        write!(out, "{name} Line")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(line: &str, station: &str, platform: &str) -> std::string::String {
        let mut s = std::string::String::new();
        write_header(&mut s, line, station, platform).unwrap();
        s
    }

    #[test]
    fn everything_known() {
        assert_eq!(
            header(
                "District",
                "East Putney Underground Station",
                "Eastbound - Platform 1"
            ),
            "District Line - East Putney Underground Station\nEastbound - Platform 1"
        );
    }

    /// The case behind this module: no predictions yet, so only the parts
    /// known from the config and the other endpoints.
    #[test]
    fn partial() {
        assert_eq!(
            header("District", "", "Platform 1"),
            "District Line\nPlatform 1"
        );
        assert_eq!(
            header("", "East Putney Underground Station", "Platform 1"),
            "East Putney Underground Station\nPlatform 1"
        );
        assert_eq!(
            header("District", "East Putney Underground Station", ""),
            "District Line - East Putney Underground Station"
        );
        assert_eq!(header("", "", "Platform 1"), "Platform 1");
        assert_eq!(header("", "", ""), "");
    }

    #[test]
    fn line_names() {
        assert_eq!(header("Elizabeth line", "", ""), "Elizabeth line");
        assert_eq!(header("DLR", "", ""), "DLR");
        assert_eq!(
            header("Hammersmith & City", "", ""),
            "Hammersmith & City Line"
        );
        assert_eq!(header("Mildmay", "", ""), "Mildmay Line");
    }
}
