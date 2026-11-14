//! TfL API parsing tests.
//!
//! This runs on the host and uses the standard library, so it cannot be built for the Pico target.
//!
//! `fixtures/` holds real API responses, refreshed with
//! `scripts/refresh-tfl-fixtures.sh`. The inline JSON cases cover edge cases
//! that real responses only produce occasionally.

use application::models::{ARRAY_MAX_SIZE_PREDICTION_MODEL, Prediction};
use application::tfl::{parse_line_status, parse_predictions};

const N: usize = ARRAY_MAX_SIZE_PREDICTION_MODEL;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}

fn times(ps: &[Prediction]) -> Vec<u32> {
    ps.iter().map(|p| p.time_to_station).collect()
}

/// Minimal prediction object; `extra` is spliced in to add or override fields.
fn prediction_json(platform: &str, tts: u32, extra: &str) -> String {
    format!(
        r#"{{"$type":"Tfl.Api.Presentation.Entities.Prediction","vehicleId":"001","stationName":"Test Station","lineName":"District","platformName":"{platform}","timestamp":"2026-11-14T05:34:35.1608646Z","timeToStation":{tts},"currentLocation":"At Platform"{extra}}}"#
    )
}

fn array(items: &[String]) -> Vec<u8> {
    format!("[{}]", items.join(",")).into_bytes()
}

// ---------------------------------------------------------------------------
// Arrivals: real responses

#[test]
fn east_putney_single_arrival() {
    let body = fixture("arrivals_east_putney.json");
    let ps = parse_predictions::<N>(&body, "Platform 2").unwrap();

    assert_eq!(ps.len(), 1);
    let p = &ps[0];
    assert_eq!(p.station_name.as_str(), "East Putney Underground Station");
    assert_eq!(p.destination_name.as_str(), "Wimbledon Underground Station");
    assert_eq!(p.platform_name.as_str(), "Westbound - Platform 2");
    assert_eq!(p.line_name.as_str(), "District");
    assert_eq!(p.time_to_station, 439);
    assert_eq!(
        p.current_location.as_str(),
        "Between West Brompton and Fulham Broadway"
    );
}

#[test]
fn east_putney_other_platform_is_empty_not_error() {
    let body = fixture("arrivals_east_putney.json");
    let ps = parse_predictions::<N>(&body, "Platform 1").unwrap();
    assert!(ps.is_empty());
}

/// Regression: the response has 12 elements. Deserialising straight into
/// `heapless::Vec<Prediction, 8>` fails with "invalid length" and drops
/// every prediction, including the ones for the configured platform.
#[test]
fn earls_court_more_arrivals_than_capacity() {
    let body = fixture("arrivals_earls_court.json");

    let naive = serde_json_core::from_slice::<heapless::Vec<Prediction, N>>(&body);
    assert!(naive.is_err(), "naive parse unexpectedly succeeded");

    let ps = parse_predictions::<N>(&body, "Platform 5").unwrap();
    assert_eq!(times(&ps), [318, 498, 1218, 1218, 1758]);
    assert!(ps.iter().all(|p| p.line_name == "Piccadilly"));
}

/// Real responses omit `destinationName` for some trains.
#[test]
fn earls_court_missing_destination_defaults() {
    let body = fixture("arrivals_earls_court.json");
    let ps = parse_predictions::<N>(&body, "Platform 1").unwrap();
    assert_eq!(times(&ps), [739, 1099]);
    assert!(ps.iter().all(|p| p.destination_name == "N/A"));
}

#[test]
fn every_fixture_parses_with_match_all_filter() {
    for name in ["arrivals_east_putney.json", "arrivals_earls_court.json"] {
        let body = fixture(name);
        let ps = parse_predictions::<64>(&body, "").unwrap();
        assert!(!ps.is_empty(), "{name}");
        assert!(ps.is_sorted_by_key(|p| p.time_to_station), "{name}");
    }
}

/// Parses whatever `scripts/refresh-tfl-fixtures.sh` last captured into
/// `fixtures/live/` (git-ignored). No-op when nothing has been captured.
#[test]
fn live_captures_parse() {
    let dir = format!("{}/tests/fixtures/live", env!("CARGO_MANIFEST_DIR"));
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let body = std::fs::read(&path).unwrap();
        if name.starts_with("arrivals_") {
            parse_predictions::<N>(&body, "").unwrap_or_else(|e| panic!("{name}: {e:?}"));
        } else if name.starts_with("status_") {
            parse_line_status(&body).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
        println!("{name}: {} bytes ok", body.len());
    }
}

// ---------------------------------------------------------------------------
// Arrivals: edge cases

#[test]
fn keeps_soonest_when_more_matches_than_capacity() {
    let items: Vec<_> = [900, 100, 800, 200, 700, 300]
        .iter()
        .map(|&t| prediction_json("Platform 1", t, ""))
        .collect();
    let ps = parse_predictions::<3>(&array(&items), "Platform 1").unwrap();
    assert_eq!(times(&ps), [100, 200, 300]);
}

#[test]
fn empty_array() {
    assert!(
        parse_predictions::<N>(b"[]", "Platform 1")
            .unwrap()
            .is_empty()
    );
    assert!(
        parse_predictions::<N>(b" [ ] \n", "Platform 1")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn null_and_missing_strings_do_not_fail() {
    let item = r#"{"timeToStation":60,"platformName":"Platform 1","destinationName":null,"currentLocation":null}"#;
    let ps = parse_predictions::<N>(&array(&[item.into()]), "Platform 1").unwrap();
    assert_eq!(ps[0].destination_name, "");
    assert_eq!(ps[0].current_location, "");
    assert_eq!(ps[0].station_name, "");
}

/// Regression for the old fixed-size fields: one long string anywhere in the
/// response used to fail the whole parse.
#[test]
fn overlong_strings_are_truncated() {
    let long = "Heathrow Terminals 2 & 3 Underground Station via Rayners Lane and Uxbridge";
    assert!(long.len() > 72);
    let item = prediction_json("Platform 1", 60, &format!(r#","destinationName":"{long}""#));
    let ps = parse_predictions::<N>(&array(&[item]), "Platform 1").unwrap();
    assert_eq!(ps[0].destination_name.as_str(), &long[..72]);
}

#[test]
fn unknown_nested_fields_are_skipped() {
    let extra = r#","timing":{"$type":"x","read":"2026-11-14T05:34:43.186Z","nested":[1,2,{"a":null}]},"operationType":1,"bearing":"""#;
    let item = prediction_json("Platform 1", 60, extra);
    let ps = parse_predictions::<N>(&array(&[item]), "Platform 1").unwrap();
    assert_eq!(ps.len(), 1);
}

/// Escapes are not decoded (see `tfl` module docs), but must not fail parsing.
#[test]
fn escaped_strings_parse() {
    let extra = r#","destinationName":"King's Cross \"St\" Pancras","towards":"a\\b\nc""#;
    let item = prediction_json("Platform 1", 60, extra);
    let ps = parse_predictions::<N>(&array(&[item]), "Platform 1").unwrap();
    assert_eq!(
        ps[0].destination_name.as_str(),
        r#"King's Cross \"St\" Pancras"#
    );
}

/// `contains` matching: "Platform 1" also matches "Platform 10"/"Platform 11".
/// Documented here so a change in behaviour is deliberate.
#[test]
fn platform_filter_is_substring_match() {
    let items = [
        prediction_json("Eastbound - Platform 1", 60, ""),
        prediction_json("Westbound - Platform 10", 120, ""),
        prediction_json("Westbound - Platform 2", 180, ""),
    ];
    let ps = parse_predictions::<N>(&array(&items), "Platform 1").unwrap();
    assert_eq!(times(&ps), [60, 120]);

    let ps = parse_predictions::<N>(&array(&items), "Eastbound").unwrap();
    assert_eq!(times(&ps), [60]);
}

#[test]
fn missing_time_to_station_is_an_error() {
    let item = r#"{"platformName":"Platform 1"}"#;
    assert!(parse_predictions::<N>(&array(&[item.into()]), "Platform 1").is_err());
}

#[test]
fn malformed_bodies_are_errors() {
    for body in [
        &b""[..],
        b"{}",
        b"[",
        b"[{\"timeToStation\":1}] trailing",
        b"<html>502 Bad Gateway</html>",
        // TfL error response, e.g. invalid api_key or rate limited
        br#"{"$type":"Tfl.Api.Presentation.Entities.ApiError","httpStatusCode":429,"message":"Too many requests"}"#,
    ] {
        assert!(
            parse_predictions::<N>(body, "Platform 1").is_err(),
            "{:?}",
            String::from_utf8_lossy(body)
        );
    }
}

/// A body truncated mid-stream (e.g. it didn't fit in the HTTP rx buffer).
#[test]
fn truncated_body_is_an_error() {
    let body = fixture("arrivals_earls_court.json");
    for cut in [1, body.len() / 2, body.len() - 1] {
        assert!(
            parse_predictions::<N>(&body[..cut], "").is_err(),
            "cut at {cut}"
        );
    }
}

// ---------------------------------------------------------------------------
// Line status

fn status_of(name: &str) -> Option<String> {
    let line = parse_line_status(&fixture(name))
        .unwrap()
        .expect("one line");
    line.line_status
        .map(|s| s.status_severity_description.as_str().to_owned())
}

#[test]
fn status_good_service() {
    assert_eq!(
        status_of("status_district_good_service.json").as_deref(),
        Some("Good Service")
    );
}

/// Real disruption payloads include long `reason` text and nested
/// `disruption` / `validityPeriods` objects that must be skipped.
#[test]
fn status_disrupted_lines() {
    assert_eq!(
        status_of("status_northern.json").as_deref(),
        Some("Part Closure")
    );
    assert_eq!(
        status_of("status_dlr.json").as_deref(),
        Some("Part Suspended")
    );
}

/// Multiple `lineStatuses`: the first one is used.
#[test]
fn status_multiple_statuses_uses_first() {
    assert_eq!(
        status_of("status_waterloo_city.json").as_deref(),
        Some("Planned Closure")
    );
}

/// Regression: these exceed the old 16-byte field and failed the whole parse.
#[test]
fn status_long_descriptions() {
    for desc in ["No Step Free Access", "Change of frequency"] {
        let body = format!(
            r#"[{{"id":"x","lineStatuses":[{{"statusSeverity":11,"statusSeverityDescription":"{desc}"}}]}}]"#
        );
        let line = parse_line_status(body.as_bytes()).unwrap().unwrap();
        assert_eq!(line.line_status.unwrap().status_severity_description, desc);
    }
}

#[test]
fn status_empty_cases() {
    assert_eq!(parse_line_status(b"[]").unwrap(), None);

    let line = parse_line_status(br#"[{"id":"x","lineStatuses":[]}]"#)
        .unwrap()
        .unwrap();
    assert_eq!(line.line_status, None);

    let line = parse_line_status(br#"[{"id":"x"}]"#).unwrap().unwrap();
    assert_eq!(line.line_status, None);
}

/// Requesting several lines at once (`/Line/a,b/Status`) uses the first.
#[test]
fn status_multiple_lines_uses_first() {
    let a = String::from_utf8(fixture("status_northern.json")).unwrap();
    let b = String::from_utf8(fixture("status_district_good_service.json")).unwrap();
    let body = format!("[{},{}]", &a[1..a.len() - 1], &b[1..b.len() - 1]);
    let line = parse_line_status(body.as_bytes()).unwrap().unwrap();
    assert_eq!(
        line.line_status.unwrap().status_severity_description,
        "Part Closure"
    );
}

#[test]
fn status_malformed_is_error() {
    assert!(parse_line_status(b"").is_err());
    assert!(parse_line_status(b"{}").is_err());
    assert!(parse_line_status(b"[{\"lineStatuses\":[").is_err());
}
