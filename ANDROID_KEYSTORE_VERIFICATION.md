# Android Keystore restart verification

This runbook verifies the secure-session lifecycle required by issue #2:

- save session on login
- survive app restart
- clear session on logout
- remain signed out after another restart

## Preconditions

1. Android emulator or device is available.
2. API is running on host port `8080`.
3. `ANDROID_NDK_HOME` is set.
4. `ANDROID_HOME` and the release signing variables are set (see the header of `scripts/build_android.sh`).

## Build and install

```bash
./scripts/build_android.sh release
APK=$(find target/dx/fingest-mobile/release/android/app -name 'fingest-release.apk' | head -n1)
adb install -r "$APK"
adb reverse tcp:8080 tcp:8080
```

## Verification steps

1. Launch app and log in with a valid account.
2. Confirm wallet list is visible.
3. Force-stop the app:
   - `adb shell am force-stop dev.fingest.mobile`
4. Relaunch app.
5. Verify the app restores the authenticated state without re-entering credentials.
6. Log out from the app.
7. Force-stop again:
   - `adb shell am force-stop dev.fingest.mobile`
8. Relaunch app.
9. Verify login screen is shown and no prior session is restored.

## Expected outcomes

- After the first restart, user remains logged in.
- After logout + restart, user is signed out.
- No plaintext session fallback is used when secure storage fails.

## Release checklist entry

Mark this item in release notes/checklist when performed:

- [ ] Android Keystore restart verification runbook executed successfully.

Record template:

- Date:
- Device/Emulator:
- APK build:
- Result: pass/fail
- Notes:
