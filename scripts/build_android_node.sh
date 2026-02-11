#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CRATE_DIR="$ROOT_DIR/apps/android-node"
ASSETS_DIR="$CRATE_DIR/src/ui/android/app/src/main/assets"

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

toolchain="${RUSTUP_TOOLCHAIN:-stable}"
export RUSTUP_TOOLCHAIN="${toolchain}"
for target in "${TARGET_LIST[@]}"; do
  echo "Ensuring Rust target ${target}"
  rustup target add "${target}"
  rustup target add "${target}" --toolchain "${RUSTUP_TOOLCHAIN}"
done

mkdir -p "$ASSETS_DIR"

for idx in "${!ABI_LIST[@]}"; do
  abi="${ABI_LIST[$idx]}"
  target="${TARGET_LIST[$idx]}"
  echo "Building android-node for $abi ($target)"
  cargo ndk -t "$abi" --manifest-path "$CRATE_DIR/Cargo.toml" build --release
  out="$ROOT_DIR/target/$target/release/veil-android-node"
  if [[ ! -f "$out" ]]; then
    echo "Missing output binary for $abi at $out"
    exit 1
  fi
  dest="$ASSETS_DIR/veil_node_${abi}"
  cp "$out" "$dest"
  echo "Wrote $dest"
done
