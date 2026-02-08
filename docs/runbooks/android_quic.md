# Android QUIC native bridge

The QUIC lane uses a Rust FFI library (`libveil_sdk_bridge.so`).
This must be built for Android ABIs and placed under the Flutter app's `jniLibs`.

## Prereqs

- Rust toolchain (`cargo`)
- Android NDK (`ANDROID_NDK_HOME` or `ANDROID_NDK_ROOT` set)
- `cargo-ndk` (auto-installed by the script)

## Build

```bash
./scripts/build_android_quic.sh
```

This generates:
```
apps/veil_android/android/app/src/main/jniLibs/<abi>/libveil_sdk_bridge.so
```

Supported ABIs (default): `arm64-v8a`, `armeabi-v7a`, `x86_64`.

## Flutter build

After building the libraries:

```bash
cd apps/veil_android
flutter run
```

If the library is missing at runtime, QUIC falls back to "unsupported" and the
app will still run.
