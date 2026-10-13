# SnipSprig

SnipSprig is a small native macOS menu-bar app. Its entire user workflow is
capture a region, keep the image above other windows, move it, and close it.

## Scope

- Keep the interface small. Do not add accounts, network calls, analytics,
  history, annotations, OCR, cloud storage, or launch-at-login without a request.
- Screen capture is user initiated. Never capture at startup or in the background.
- Never bypass macOS screen-recording permission or change privacy settings.
- Screenshot data stays local. Delete temporary captures as soon as they are read.
- Never commit actual screenshots, secrets, signed credentials, or build outputs.

## Layout

- `Sources/SnipSprig/`: AppKit UI, menu-bar lifecycle, and Carbon hotkey adapter.
- `Sources/SnipSprigKit/`: capture process lifecycle and pure window geometry.
- `Tests/SnipSprigKitTests/`: deterministic behavior and capture cleanup checks.
- `Resources/Info.plist`: stable bundle identity and minimum macOS version.
- `scripts/build.sh`: reproducible local app packaging and ad-hoc signing.
- `scripts/test.sh`: portable Swift Testing runner, including CLT macro loading.
- `scripts/make-icon.swift`: programmatic app icon, no downloaded assets.
- `.github/workflows/ci.yml`: tests and packaging on macOS.

## Development

Requires macOS 13+ and Swift 6+ from Xcode or Command Line Tools.

```sh
bash scripts/test.sh
swift build -c release
bash scripts/build.sh
open dist/SnipSprig.app
```

Use Swift 6 concurrency checking. Keep AppKit state on `@MainActor`.
Capture must not block the main run loop. Keep subprocess arguments as an array;
never invoke a shell with interpolated user data. Retain a single capture at a
time. Clean temporary directories on cancellation, errors, and successful capture.

Use focused file patches for text edits. Keep changes confined to the request.
Add tests for behavioral risks such as cancellation, cleanup, off-screen window
placement, and negative multi-monitor coordinates; do not mirror UI implementation.

## Validation and delivery

Run `bash scripts/test.sh` and `bash scripts/build.sh` after code changes. Check
`git diff --check`. Verify the app bundle with `codesign --verify --strict`.
For UI changes, launch the app and inspect the real window. `--demo` uses a
generated reference image to check floating windows without capturing private
screen content. Test real capture only after the user grants screen-recording
access. Report clearly if a live capture was not verified.

Keep the bundle identifier `io.github.rensilin.snipsprig` stable across updates so
screen-recording permission is associated with the same app identity.
Local builds are ad-hoc signed, not Apple-notarized distribution builds.

Do not push or publish changes unless the user has authorized that work. This
initial task explicitly authorizes creation and pushing of this GitHub repository.
