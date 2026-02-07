# Self-hosted F-Droid repo (GitHub Pages)

This repo supports publishing a self-hosted F-Droid repository to GitHub Pages via CI.

## Required secrets (GitHub Actions)

Android app signing (release APK):
- `ANDROID_KEYSTORE_B64` — base64-encoded keystore
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

F-Droid repo signing (index signing):
- `FDROID_KEYSTORE_B64` — base64-encoded keystore
- `FDROID_KEYSTORE_PASS`
- `FDROID_KEY_ALIAS`
- `FDROID_KEY_PASS`

You can reuse the Android keystore for repo signing, but a separate repo key is recommended.

## Repo URL

GitHub Pages repo URL is derived automatically:

```
https://<owner>.github.io/<repo>/repo
```

## Publish workflow

The workflow builds a signed release APK, creates the F-Droid index, and publishes
`fdroid/` to the `gh-pages` branch. The F-Droid repo lives at `/repo`.

## Local test (optional)

```bash
cd /path/to/veil
export FDROID_REPO_URL="https://example.com/repo"
export FDROID_KEYSTORE_PASS="..."
export FDROID_KEY_ALIAS="..."
export FDROID_KEY_PASS="..."
./scripts/fdroid/build_repo.sh
```

## Notes

- Metadata is sourced from `metadata/app.veil.android`.
- The APK path defaults to Flutter’s `app-release.apk`.
