#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$project_root"

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'cargo not found. Install Rust via https://rustup.rs/ and retry.' >&2
    exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
    printf '%s\n' 'rustup not found. Install Rust via https://rustup.rs/ and retry.' >&2
    exit 1
fi

if ! command -v zip >/dev/null 2>&1; then
    printf '%s\n' 'zip not found. Install the zip package and retry.' >&2
    exit 1
fi

rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-gnu

mkdir -p dist
tar -C target/x86_64-unknown-linux-gnu/release -czf dist/lupus-linux-x86_64.tar.gz lupus
zip -j dist/lupus-windows-x86_64.zip target/x86_64-pc-windows-gnu/release/lupus.exe

printf '%s\n' \
    "Linux:   $project_root/target/x86_64-unknown-linux-gnu/release/lupus" \
    "Windows: $project_root/target/x86_64-pc-windows-gnu/release/lupus.exe" \
    "Archives: $project_root/dist/lupus-linux-x86_64.tar.gz" \
    "         $project_root/dist/lupus-windows-x86_64.zip"