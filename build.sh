#!/bin/bash
# Build + sign NovaKey.app (macOS).
#
# Signing modes (auto-selected):
#   ad-hoc   (default) no env set -> local dev build, NOT distributable
#   release  set SIGN_IDENTITY to a "Developer ID Application: ..." identity
#
# Notarization (release only): set NOTARIZE=1 plus ONE credential set:
#   A) NOTARY_PROFILE                          notarytool keychain profile
#   B) APPLE_API_KEY / APPLE_API_KEY_ID / APPLE_API_ISSUER    App Store Connect API key (.p8)
#   C) APPLE_ID / APPLE_TEAM_ID / APPLE_APP_SPECIFIC_PASSWORD  Apple ID + app-specific password
#
# Examples:
#   ./build.sh                                   # ad-hoc dev build
#   SIGN_IDENTITY="Developer ID Application: Acme (TEAMID)" ./build.sh
#   SIGN_IDENTITY="..." NOTARIZE=1 NOTARY_PROFILE=novakey ./build.sh
set -euo pipefail
cd "$(dirname "$0")"

APP="build/NovaKey.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"
BIN="$MACOS/NovaKey"
ENTITLEMENTS="Resources/NovaKey.entitlements"

echo "▶ Building..."
swift build -c release

echo "▶ Assembling app bundle..."
rm -rf "$APP"
mkdir -p "$MACOS" "$RESOURCES"

# Binary
cp .build/release/NovaKey "$BIN"
chmod +x "$BIN"

# All resources
cp Resources/Info.plist            "$CONTENTS/Info.plist"
cp Resources/AppIcon.icns          "$RESOURCES/AppIcon.icns"
cp Resources/AppLogo.png           "$RESOURCES/AppLogo.png"
cp "$ENTITLEMENTS"                  "$RESOURCES/NovaKey.entitlements"

# ── Sign ──────────────────────────────────────────────────────────────────
if [[ -n "${SIGN_IDENTITY:-}" ]]; then
    echo "▶ Signing (Developer ID): $SIGN_IDENTITY"
    SIGN_ARGS=(--force --sign "$SIGN_IDENTITY" --options runtime --timestamp
               --entitlements "$ENTITLEMENTS")
    # inside-out: nested code first, then the bundle. No --deep (Apple-discouraged).
    codesign "${SIGN_ARGS[@]}" "$BIN"
    codesign "${SIGN_ARGS[@]}" "$APP"
    ADHOC=0
else
    echo "▶ Signing (ad-hoc — local dev only, not distributable)..."
    codesign --force --sign - --entitlements "$ENTITLEMENTS" --options runtime "$BIN"
    codesign --force --sign - --entitlements "$ENTITLEMENTS" --options runtime "$APP"
    ADHOC=1
fi

echo "▶ Verifying signature..."
codesign --verify --strict --verbose=2 "$APP"
echo "  Signature OK"

# ── Notarize + staple ───────────────────────────────────────────────────────
if [[ "${NOTARIZE:-0}" == "1" ]]; then
    if [[ "$ADHOC" == "1" ]]; then
        echo "✗ NOTARIZE=1 requires a real SIGN_IDENTITY (ad-hoc cannot be notarized)." >&2
        exit 1
    fi

    if [[ -n "${NOTARY_PROFILE:-}" ]]; then
        NOTARY_AUTH=(--keychain-profile "$NOTARY_PROFILE")
    elif [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER:-}" ]]; then
        NOTARY_AUTH=(--key "$APPLE_API_KEY" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER")
    elif [[ -n "${APPLE_ID:-}" && -n "${APPLE_TEAM_ID:-}" && -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]]; then
        NOTARY_AUTH=(--apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD")
    else
        echo "✗ NOTARIZE=1 but no credentials. Set NOTARY_PROFILE, or APPLE_API_* , or APPLE_ID/APPLE_TEAM_ID/APPLE_APP_SPECIFIC_PASSWORD." >&2
        exit 1
    fi

    ZIP="build/NovaKey.zip"
    echo "▶ Zipping for notarization..."
    /usr/bin/ditto -c -k --keepParent "$APP" "$ZIP"

    echo "▶ Submitting to Apple notary (waits for result)..."
    xcrun notarytool submit "$ZIP" "${NOTARY_AUTH[@]}" --wait

    echo "▶ Stapling ticket..."
    xcrun stapler staple "$APP"
    xcrun stapler validate "$APP"
    rm -f "$ZIP"
    echo "  Notarized + stapled"
fi

echo "  Bundle ID: $(defaults read "$(pwd)/$CONTENTS/Info" CFBundleIdentifier)"
echo "  Version:   $(defaults read "$(pwd)/$CONTENTS/Info" CFBundleShortVersionString)"
echo "✓ Done: $APP"
