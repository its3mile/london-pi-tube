#!/usr/bin/env bash
# Re-capture real TfL API responses used by application's parsing tests.
#
# Run this when production parsing misbehaves, or periodically to catch API
# schema drift, then `cargo test`. Tests that assert exact values (times,
# platforms) are tied to the captured data and will need updating; all other
# tests should pass against any fresh capture.
#
# Usage: scripts/refresh-tfl-fixtures.sh [api_key]
set -euo pipefail

dir="$(cd "$(dirname "$0")/.." && pwd)/application/tests/fixtures/live"
mkdir -p "$dir"
key="${1:-}"
q="${key:+?app_key=$key}"

fetch() { # name path
    curl -fsS --max-time 20 "https://api.tfl.gov.uk/$2$q" -o "$dir/$1.json"
    echo "$1: $(wc -c <"$dir/$1.json") bytes"
}

fetch arrivals_east_putney StopPoint/940GZZLUEPY/Arrivals
fetch arrivals_earls_court  StopPoint/940GZZLUECT/Arrivals
fetch arrivals_kings_cross  StopPoint/940GZZLUKSX/Arrivals
fetch status_district       Line/district/Status
fetch status_all            Line/Mode/tube,elizabeth-line,overground,dlr/Status
