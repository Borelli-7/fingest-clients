#!/usr/bin/env bash
# Builds the Android APK with the Keystore dependency dx does not know about.
#
# `dx` generates app/build.gradle.kts from a fixed template with no project-local override,
# so androidx.security:security-crypto — which provides EncryptedSharedPreferences — has to
# be injected after generation and before the Gradle assemble.
#
# Without it the app still runs: AndroidKeystore fails to find the class, reports
# SecretError::Unavailable, and the session falls back to memory. That is the intended
# fail-closed path, not a crash — but the session will not survive a restart.
set -euo pipefail

PROFILE="${1:-release}"
CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$CRATE_DIR/bins/fingest-mobile"
SECURITY_DEP='implementation("androidx.security:security-crypto:1.1.0")'

: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME (see ~/.zshrc)}"

build_flag=""
[ "$PROFILE" = "release" ] && build_flag="--release"

cd "$BIN_DIR"

# First pass: generates the Gradle project and builds an APK without the dependency.
dx build --android $build_flag

GRADLE_FILE="$CRATE_DIR/target/dx/fingest-mobile/$PROFILE/android/app/app/build.gradle.kts"
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

# Second pass: re-assemble so the dependency is actually linked in.
cd "$CRATE_DIR/target/dx/fingest-mobile/$PROFILE/android/app"
./gradlew assembleDebug --console=plain -q

find . -name '*.apk' -newer "$GRADLE_FILE" -printf '%s\t%p\n' |
    awk -F'\t' '{printf "%.1f MB  %s\n", $1/1048576, $2}'
