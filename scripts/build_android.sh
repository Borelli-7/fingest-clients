#!/usr/bin/env bash
# Builds the Android APK with the Keystore dependency dx does not know about.
#
#   ./scripts/build_android.sh release   # signed, non-debuggable
#   ./scripts/build_android.sh debug     # debug-signed, debuggable, for development only
#
# `dx` generates app/build.gradle.kts from a fixed template with no project-local override,
# so androidx.security:security-crypto — which provides EncryptedSharedPreferences — has to
# be injected after generation and before the Gradle assemble.
#
# Without it the app still runs: AndroidKeystore fails to find the class, reports
# SecretError::Unavailable, and the session falls back to memory. That is the intended
# fail-closed path, not a crash — but the session will not survive a restart.
#
# A release build needs a signing key, passed through the environment so no password ever
# appears on a command line:
#   FINGEST_KEYSTORE        path to the keystore
#   FINGEST_KEY_ALIAS       key alias inside it
#   FINGEST_KEYSTORE_PASS   keystore password
#   FINGEST_KEY_PASS        key password (optional, defaults to the keystore password)
set -euo pipefail

PROFILE="${1:-release}"
case "$PROFILE" in
    release | debug) ;;
    *) echo "usage: $0 [release|debug]" >&2; exit 2 ;;
esac

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$CRATE_DIR/bins/fingest-mobile"
SECURITY_DEP='implementation("androidx.security:security-crypto:1.1.0")'

: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME (see ~/.zshrc)}"

if [ "$PROFILE" = "release" ]; then
    # Fail before a long build rather than after it.
    : "${FINGEST_KEYSTORE:?release builds need FINGEST_KEYSTORE (see the header of this script)}"
    : "${FINGEST_KEY_ALIAS:?release builds need FINGEST_KEY_ALIAS}"
    : "${FINGEST_KEYSTORE_PASS:?release builds need FINGEST_KEYSTORE_PASS}"
    [ -f "$FINGEST_KEYSTORE" ] || { echo "no keystore at $FINGEST_KEYSTORE" >&2; exit 1; }

    : "${ANDROID_HOME:?release builds need ANDROID_HOME to find apksigner and zipalign}"
    BUILD_TOOLS="${ANDROID_BUILD_TOOLS:-$(find "$ANDROID_HOME/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n1)}"
    for tool in zipalign apksigner aapt2; do
        [ -x "$BUILD_TOOLS/$tool" ] || { echo "missing $tool in $BUILD_TOOLS" >&2; exit 1; }
    done
fi

build_flag=""
[ "$PROFILE" = "release" ] && build_flag="--release"

cd "$BIN_DIR"

# First pass: generates the Gradle project and builds an APK without the dependency.
dx build --android $build_flag

APP_DIR="$CRATE_DIR/target/dx/fingest-mobile/$PROFILE/android/app"
GRADLE_FILE="$APP_DIR/app/build.gradle.kts"
[ -f "$GRADLE_FILE" ] || { echo "no generated gradle file at $GRADLE_FILE" >&2; exit 1; }

if grep -qF "security-crypto" "$GRADLE_FILE"; then
    echo "security-crypto already present"
else
    # Append inside the existing dependencies block.
    awk -v dep="    $SECURITY_DEP" '
        /^dependencies \{/ { print; print dep; next }
        { print }
    ' "$GRADLE_FILE" > "$GRADLE_FILE.tmp" && mv "$GRADLE_FILE.tmp" "$GRADLE_FILE"
    echo "injected $SECURITY_DEP"
fi

# The release build type minifies. R8 cannot see classes reached only through JNI, so it
# would strip the Keystore adapter's targets and silently disable persistence. The template
# already applies every *.pro file under app/, so a rules file there is picked up.
cat > "$APP_DIR/app/fingest-keystore.pro" <<'EOF'
# Reached only from Rust through JNI (crates/fingest-mobile-storage/src/android.rs).
-keep class androidx.security.crypto.** { *; }
# Tink (under security-crypto) references compile-only annotations absent at runtime.
-dontwarn javax.annotation.Nullable
-dontwarn javax.annotation.concurrent.GuardedBy
EOF

# Second pass: re-assemble so the dependency is actually linked in.
cd "$APP_DIR"

if [ "$PROFILE" = "debug" ]; then
    ./gradlew assembleDebug --console=plain -q
    APK="$APP_DIR/app/build/outputs/apk/debug/app-debug.apk"
else
    ./gradlew assembleRelease --console=plain -q

    OUT="$APP_DIR/app/build/outputs/apk/release"
    UNSIGNED="$OUT/app-release-unsigned.apk"
    [ -f "$UNSIGNED" ] || { echo "no unsigned release APK at $UNSIGNED" >&2; exit 1; }

    APK="$OUT/fingest-release.apk"
    "$BUILD_TOOLS/zipalign" -f -p 4 "$UNSIGNED" "$OUT/app-release-aligned.apk"

    key_pass=(--key-pass "env:FINGEST_KEYSTORE_PASS")
    [ -n "${FINGEST_KEY_PASS:-}" ] && key_pass=(--key-pass "env:FINGEST_KEY_PASS")
    "$BUILD_TOOLS/apksigner" sign \
        --ks "$FINGEST_KEYSTORE" \
        --ks-key-alias "$FINGEST_KEY_ALIAS" \
        --ks-pass "env:FINGEST_KEYSTORE_PASS" \
        "${key_pass[@]}" \
        --out "$APK" \
        "$OUT/app-release-aligned.apk"
    rm -f "$OUT/app-release-aligned.apk"

    "$BUILD_TOOLS/apksigner" verify "$APK"

    # A debuggable APK exposes app-private storage through `run-as`; never hand one out as release.
    if "$BUILD_TOOLS/aapt2" dump xmltree --file AndroidManifest.xml "$APK" | grep -q 'debuggable.*=true'; then
        echo "refusing: $APK is debuggable" >&2
        exit 1
    fi
fi

printf '%s\t%s\n' "$(stat -c %s "$APK")" "$APK" |
    awk -F'\t' '{printf "%.1f MB  %s\n", $1/1048576, $2}'
