#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=${FDROID_ROOT:-fdroid}
APP_ID=${FDROID_APP_ID:-app.veil.android}
APK_PATH=${APK_PATH:-apps/veil_android/build/app/outputs/flutter-apk/app-release.apk}
REPO_URL=${FDROID_REPO_URL:-}
REPO_NAME=${FDROID_REPO_NAME:-VEIL F-Droid Repo}
REPO_DESC=${FDROID_REPO_DESC:-VEIL Android builds}
REPO_ICON=${FDROID_REPO_ICON:-veil_logo.png}

mkdir -p "$REPO_ROOT/repo" "$REPO_ROOT/metadata" "$REPO_ROOT/repo/icons"

if [[ -d "metadata/${APP_ID}" ]]; then
  rm -rf "$REPO_ROOT/metadata/${APP_ID}"
  cp -a "metadata/${APP_ID}" "$REPO_ROOT/metadata/${APP_ID}"
fi

if [[ ! -f "$APK_PATH" ]]; then
  echo "APK not found at $APK_PATH"
  exit 1
fi

cp -f "$APK_PATH" "$REPO_ROOT/repo/${APP_ID}.apk"

if [[ -z "$REPO_URL" ]]; then
  echo "FDROID_REPO_URL is required"
  exit 1
fi

if [[ -z "${FDROID_KEYSTORE_PASS:-}" || -z "${FDROID_KEY_ALIAS:-}" || -z "${FDROID_KEY_PASS:-}" ]]; then
  echo "FDROID_KEYSTORE_PASS, FDROID_KEY_ALIAS, and FDROID_KEY_PASS are required"
  exit 1
fi

cat > "$REPO_ROOT/config.yml" <<CONFIG
repo_url: "$REPO_URL"
repo_name: "$REPO_NAME"
repo_description: "$REPO_DESC"
repo_icon: "icons/icon.png"
archive_older: 0

keystore: "$REPO_ROOT/keystore.jks"
keystorepass: "${FDROID_KEYSTORE_PASS}"
keypass: "${FDROID_KEY_PASS}"
repo_keyalias: "${FDROID_KEY_ALIAS}"
CONFIG

if [[ -f "$REPO_ICON" ]]; then
  cp -f "$REPO_ICON" "$REPO_ROOT/repo/icons/icon.png"
fi

pushd "$REPO_ROOT" >/dev/null
fdroid update --create-metadata --verbose --clean
popd >/dev/null
