#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CRATE_DIR="$ROOT_DIR/packages/veil-sdk-dart/rust"
OUT_DIR="$ROOT_DIR/apps/veil_android/android/app/src/main/jniLibs"

ABI_LIST=("arm64-v8a" "armeabi-v7a" "x86_64")
TARGET_LIST=("aarch64-linux-android" "armv7-linux-androideabi" "x86_64-linux-android")

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found; install Rust first"
  exit 1
fi

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk not found; installing..."
  cargo install cargo-ndk --locked
fi

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found; install Rust with rustup first."
  exit 1
fi

if [[ -z "${ANDROID_NDK_HOME:-}" && -z "${ANDROID_NDK_ROOT:-}" ]]; then
  if [[ -n "${ANDROID_SDK_ROOT:-}" && -d "${ANDROID_SDK_ROOT}/ndk" ]]; then
    latest_ndk=$(ls -1 "${ANDROID_SDK_ROOT}/ndk" 2>/dev/null | sort -V | tail -n 1)
    if [[ -n "${latest_ndk:-}" && -d "${ANDROID_SDK_ROOT}/ndk/${latest_ndk}" ]]; then
      export ANDROID_NDK_HOME="${ANDROID_SDK_ROOT}/ndk/${latest_ndk}"
    fi
  fi
fi

if [[ -z "${ANDROID_NDK_HOME:-}" && -z "${ANDROID_NDK_ROOT:-}" ]]; then
  echo "ANDROID_NDK_HOME/ANDROID_NDK_ROOT not set."
  echo "Tip: export ANDROID_SDK_ROOT and re-run, or set ANDROID_NDK_HOME directly."
  exit 1
fi

NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT}}"

toolchain="$(rustup show active-toolchain 2>/dev/null | awk '{print $1}')"
if [[ -z "${toolchain}" ]]; then
  toolchain="stable"
fi
for target in "${TARGET_LIST[@]}"; do
  echo "Ensuring Rust target ${target} (${toolchain})"
  rustup target add "${target}" --toolchain "${toolchain}"
done

for abi in "${ABI_LIST[@]}"; do
  echo "Building QUIC bridge for $abi"
  cargo ndk -o "$OUT_DIR" -t "$abi" --manifest-path "$CRATE_DIR/Cargo.toml" build --release
  if [[ ! -f "$OUT_DIR/$abi/libveil_sdk_bridge.so" ]]; then
    echo "Missing output for $abi"
    exit 1
  fi
  echo "Wrote $OUT_DIR/$abi/libveil_sdk_bridge.so"
  done
