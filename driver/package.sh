#!/usr/bin/env bash
# Build the DriverWorks .c4z (a plain zip).
set -euo pipefail
cd "$(dirname "$0")"
out="c4-nav-sink.c4z"
rm -f "$out"
zip -r -X "$out" driver.xml driver.lua www >/dev/null
echo "built $(pwd)/$out"; unzip -l "$out"
