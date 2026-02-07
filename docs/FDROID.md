# F-Droid Release Notes

This repository contains the VEIL Android app under `apps/veil_android`.

## Package ID

- `app.veil.android`

## Build Steps

From repo root:

```bash
cd apps/veil_android
flutter pub get
flutter build apk --release
```

F-Droid will build from source and sign releases with its own keys.

## Requirements

- No proprietary services (no Google Play Services, no Firebase).
- Open-source dependencies only.
- Network access is limited to app functionality; no analytics/tracking.

## Notes

- The app supports local relay mode by default for development and demo use.
- Bluetooth requires runtime permission on Android 12+.
