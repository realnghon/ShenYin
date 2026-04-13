# 2026-03-30 ShenYin macOS ARM64 GitHub Actions build design

## Summary

Add a separate macOS GitHub Actions build job that produces an unsigned Apple Silicon `.app`, packages it as a `.zip`, runs a smoke test, and publishes it alongside the existing Windows `.exe`. Keep the current encryption protocol unchanged so data encrypted on Windows can be decrypted on macOS and vice versa.

## Goals

- Keep the existing Windows release flow working
- Add a separate `macos-14` CI job for Apple Silicon builds
- Publish a macOS `.app` packaged as a `.zip`
- Preserve bidirectional cross-platform decrypt compatibility
- Avoid Apple signing and notarization for now

## Non-goals

- No Apple code signing or notarization
- No changes to the encryption format, versioning, or transport encoding
- No UI feature changes
- No refactor of the core crypto or Flask application architecture beyond what packaging requires
- No conversion of the whole workflow into a matrix build

## Current state

### Release pipeline

The repository currently has a single GitHub Actions workflow at `.github/workflows/build-and-release.yml` with one Windows-only job:

- runs on `windows-latest`
- creates a virtual environment
- installs dependencies and `pyinstaller`
- runs unit tests
- builds `dist/ShenYin.exe` via `build_exe.bat`
- smoke tests the executable by starting the local server and checking `GET /`
- uploads the Windows artifact and publishes it to prerelease / tagged releases

### Packaging configuration

Packaging is currently driven by:

- `build_exe.bat`
- `local-workspace.spec`

The spec already centralizes most PyInstaller configuration and includes:

- `app.py` as the entry point
- `templates/` and `static/` as bundled data
- windowed application mode (`console=False`)

This is a good base for macOS packaging because PyInstaller can use the same spec on macOS to produce an `.app` bundle.

### Encryption compatibility baseline

Cross-platform data compatibility already depends on the pure-Python engine, not on the packaging target:

- `pgpbox/engine.py` defines the encrypted payload format
- AES-256-GCM + PBKDF2-HMAC-SHA256 are implemented in Python via `cryptography`
- the payload embeds version byte, salt, nonce, metadata length, metadata JSON, and ciphertext
- text transport encoding is handled separately and should remain unchanged

As long as the protocol and transport remain unchanged, Windows and macOS builds will remain interoperable.

## Recommended approach

Add a second, independent macOS job rather than converting the workflow to a platform matrix.

### Why this approach

- It minimizes change to the existing stable Windows pipeline
- It keeps platform-specific troubleshooting isolated
- It avoids mixing release and smoke-test logic for different operating systems into one abstracted job too early
- It is the smallest change that satisfies the requested outcome

## Design

### 1. Workflow structure

Keep the existing Windows build job and add a new independent macOS build job in the same workflow.

Planned job layout:

1. `build-windows`
2. `build-macos-arm64`
3. `release`

The `release` job is required in this design. Both platform build jobs should upload artifacts only, and the dedicated `release` job should download both artifacts and publish them together.

The macOS job will:

- run on `macos-14`
- use Python 3.10
- install project dependencies and `pyinstaller`
- run the same unit test suite
- build `dist/ShenYin.app`
- run a smoke test against the packaged app
- compress the app bundle into a zip archive
- upload the zip as a workflow artifact

The release job will:

- depend on both platform build jobs
- download the Windows and macOS artifacts
- publish both assets to the prerelease or tagged release together

### 2. Packaging strategy

Continue using PyInstaller.

#### Shared spec

Use `local-workspace.spec` as the primary packaging configuration for both platforms unless a minimal platform-specific adjustment becomes necessary.

The existing spec is already suitable for a GUI-style Flask launcher:

- entry point remains `app.py`
- bundled resources remain `templates/` and `static/`
- application name remains `ShenYin`

On macOS, PyInstaller should produce `dist/ShenYin.app` from the same spec.

#### macOS build entrypoint

Add a dedicated shell script such as `build_macos.sh` to keep macOS build commands out of the Windows batch file.

Responsibilities:

- verify the expected Python environment exists
- optionally remove prior `build/` and `dist/` outputs
- invoke PyInstaller against `local-workspace.spec`
- fail fast on packaging errors

This keeps platform differences inside the build layer and avoids overloading `build_exe.bat` with cross-platform branching.

### 3. macOS smoke test

Mirror the existing Windows smoke test as closely as possible.

Proposed flow:

- launch `dist/ShenYin.app/Contents/MacOS/ShenYin`
- pass `--host 127.0.0.1 --port 19876 --no-browser`
- wait briefly for startup
- request `http://127.0.0.1:19876/`
- require HTTP 200
- stop the process in a finally/cleanup step

This command-line shape is the required baseline for the smoke test so the packaged app is validated in the same headless style as the existing Windows smoke test. Small mechanical shell differences are acceptable, but the smoke test should launch the packaged binary directly, disable automatic browser opening, bind an explicit local host/port, and verify the root page responds with HTTP 200.

### 4. Artifact naming and release outputs

Keep naming explicit by platform.

Recommended outputs:

- Windows artifact / release file: `ShenYin.exe`
- macOS archive / release file: `ShenYin-macos-arm64.zip`

Recommended artifact names:

- `ShenYin-win-x64-${{ github.ref_name }}`
- `ShenYin-macos-arm64-${{ github.ref_name }}`

The macOS job should zip the `.app` bundle before upload and release publication, since `.app` is a directory bundle and should not be uploaded raw.

### 5. Release model

Use one workflow to produce both platform artifacts and publish them together.

Preferred design:

- build jobs upload artifacts only
- release job downloads both artifacts
- release job publishes a single prerelease or tagged release with both files attached

Why:

- avoids races where two jobs update the same release simultaneously
- keeps release logic in one place
- ensures incomplete builds do not publish a partial release

### 6. Compatibility guarantees

To preserve Windows/macOS interoperability, the implementation must not change:

- `pgpbox/engine.py` payload version byte
- metadata structure and field names
- PBKDF2 iteration count, digest, or key length
- AES-GCM nonce/salt framing
- Base85 transport behavior for text mode

The packaging work is intentionally isolated from the crypto protocol.

### 7. README changes

Update `README.md` so the download and release instructions reflect both platforms.

Required updates:

- mention both Windows and macOS downloads
- note that macOS builds target Apple Silicon (M1 and newer)
- note that the macOS app is unsigned, so first launch may require manual approval in system security settings
- keep messaging concise and user-focused

## Error handling

Implementation should only add handling directly relevant to packaging and release reliability:

- fail the macOS job if packaging fails
- fail the macOS job if the smoke test fails
- fail the release job if one expected artifact is missing

Do not expand scope into unrelated runtime error handling inside the app.

## Testing strategy

### Required CI validation

For both platforms where applicable:

- install dependencies successfully
- run `python -m unittest discover -s tests -v`
- run packaged smoke test successfully

### Compatibility protection

The main compatibility guarantee comes from preserving and continuing to test the existing crypto code path.

At minimum:

- keep existing crypto tests passing
- avoid any protocol-level edits in `pgpbox/engine.py` and transport helpers

If a small additional test is needed, it should focus on round-trip encryption/decryption behavior without introducing artificial platform-specific test complexity.

## Risks and mitigations

### Risk: macOS packaging differs subtly from Windows packaging

**Mitigation:** Keep the spec shared where possible and add a macOS smoke test against the packaged app bundle.

### Risk: release publication races or incomplete assets

**Mitigation:** Publish from a single release job after both platform builds succeed and artifacts are available.

### Risk: first-run friction on macOS due to lack of signing

**Mitigation:** Document clearly in the README and release notes that the app is unsigned and may require manual approval on first launch.

## Implementation boundaries

Expected files to change:

- `.github/workflows/build-and-release.yml`
- `README.md`
- new macOS build helper script such as `build_macos.sh`
- possibly packaging-related files if minor adjustments are required for PyInstaller on macOS
- optionally a small test update if needed to protect compatibility assumptions

Expected files not to change for feature scope reasons:

- `pgpbox/engine.py` protocol design
- `pgpbox/crypto.py` behavior except if packaging discovery requires a tiny non-protocol adjustment
- Flask routes and UI behavior

## Success criteria

The work is successful when all of the following are true:

1. Pushing to `main` still produces the Windows executable release flow
2. The workflow also builds an Apple Silicon macOS app on GitHub Actions
3. The macOS job publishes a zipped `.app` artifact
4. Tagged releases contain both Windows and macOS assets
5. The macOS packaged app passes a smoke test in CI
6. Existing encrypted content format remains unchanged
7. Content encrypted on one platform can be decrypted on the other because both builds use the same unchanged crypto protocol
