#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

mkdir -p docs/screenshots
cargo build --release

BIN=./target/release/lunar

LUNAR_SCENE=approach \
LUNAR_SCREENSHOT=docs/screenshots/approach.png \
LUNAR_SCREENSHOT_FRAME=90 \
"$BIN"

LUNAR_SCENE=autopilot \
LUNAR_WARMUP_STEPS=4200 \
LUNAR_SCREENSHOT=docs/screenshots/autopilot.png \
LUNAR_SCREENSHOT_FRAME=60 \
"$BIN"

LUNAR_SCENE=landed \
LUNAR_SCREENSHOT=docs/screenshots/landed.png \
LUNAR_SCREENSHOT_FRAME=90 \
"$BIN"

echo "Screenshots written to docs/screenshots/"
