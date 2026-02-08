#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CRATE_DIR="$ROOT_DIR/packages/veil-sdk-dart/rust"
OUT_DIR="$ROOT_DIR/apps/veil_android/android/app/src/main/jniLibs"

ABI_LIST=("arm64-v8a" "armeabi-v7a" "x86_64")

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found; install Rust first"
  exit 1
fi

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk not found; installing..."
  cargo install cargo-ndk --locked
fi

if [[ -z "${ANDROID_NDK_HOME:-}" && -z "${ANDROID_NDK_ROOT:-}" ]]; then
  echo "ANDROID_NDK_HOME/ANDROID_NDK_ROOT not set."
  exit 1
fi

NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT}}"

for abi in "${ABI_LIST[@]}"; do
  echo "Building QUIC bridge for $abi"
  cargo ndk -o "$OUT_DIR" -t "$abi" --manifest-path "$CRATE_DIR/Cargo.toml" build --release
  if [[ ! -f "$OUT_DIR/$abi/libveil_sdk_bridge.so" ]]; then
    echo "Missing output for $abi"
    exit 1
  fi
  echo "Wrote $OUT_DIR/$abi/libveil_sdk_bridge.so"
  done

