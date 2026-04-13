# ShenYin macOS ARM64 Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a separate GitHub Actions macOS ARM64 build that packages `ShenYin.app` as a zip, smoke-tests it, and publishes it alongside the existing Windows executable without changing the crypto protocol.

**Architecture:** Keep packaging changes isolated to build/release tooling. Reuse the existing PyInstaller spec where possible, add a macOS-specific build helper, split CI into independent Windows and macOS build jobs, and publish both assets from a required release job. Preserve cross-platform decrypt compatibility by not changing the engine or transport protocol.

**Tech Stack:** GitHub Actions, Python 3.10, PyInstaller, unittest, PowerShell, bash, macOS 14 runner

---

## File map

- Modify: `.github/workflows/build-and-release.yml`
  - Split the current Windows-only flow into explicit `build-windows`, `build-macos-arm64`, and `release` jobs.
  - Keep existing `main` and tag triggers unchanged.
  - Upload platform artifacts from build jobs and publish from the release job.
- Create: `build_macos.sh`
  - macOS-only helper that runs PyInstaller against `local-workspace.spec` and fails fast.
- Modify: `local-workspace.spec`
  - Only if required for macOS bundle correctness while preserving the current Windows build.
- Modify: `README.md`
  - Document Windows + macOS downloads and unsigned macOS first-run behavior.
- Modify: `tests/test_crypto.py`
  - Only if a tiny compatibility-protection assertion is needed; avoid protocol edits.

## Task 1: Add a macOS build helper

**Files:**
- Create: `build_macos.sh`
- Modify: `local-workspace.spec` (only if needed after a failing build)
- Test: `.github/workflows/build-and-release.yml`

- [ ] **Step 1: Write the helper script content**

```bash
#!/usr/bin/env bash
set -euo pipefail

if [ ! -x ".venv/bin/python" ]; then
  echo "Please create the venv first."
  exit 1
fi

rm -rf build dist
".venv/bin/python" -m pip install pyinstaller
".venv/bin/pyinstaller" --noconfirm --clean local-workspace.spec
```

Expected behavior:
- Uses the macOS venv layout (`.venv/bin/...`)
- Rebuilds from a clean `build/` and `dist/`
- Produces `dist/ShenYin.app` on macOS

- [ ] **Step 2: Save the script and make it executable in git**

Run:
```bash
chmod +x build_macos.sh
```

Expected: `git diff --summary` later shows mode change for `build_macos.sh`

- [ ] **Step 3: If macOS packaging fails, inspect whether `local-workspace.spec` needs a minimal adjustment**

Check:
- app name remains `ShenYin`
- entry point remains `app.py`
- bundled data stays `templates/` and `static/`
- no Windows-specific setting was accidentally introduced

Expected: no `.spec` change unless the macOS CI run demonstrates a real need.

- [ ] **Step 4: Commit the helper script work**

```bash
git add build_macos.sh local-workspace.spec
git commit -m "build: add macOS packaging helper"
```

## Task 2: Restructure the workflow into separate build and release jobs

**Files:**
- Modify: `.github/workflows/build-and-release.yml`
- Test: `.github/workflows/build-and-release.yml`

- [ ] **Step 1: Rename the existing job to `build-windows` and preserve its current behavior**

Edit `.github/workflows/build-and-release.yml` so the current Windows sequence keeps:
- `runs-on: windows-latest`
- virtualenv creation
- dependency install
- unit tests
- `build_exe.bat`
- EXE smoke test

Expected: no functional change to the Windows packaging path yet.

- [ ] **Step 2: Remove release publication from the Windows build job**

Delete the current release-writing steps from the Windows job:
- `Move Latest Tag`
- `Publish Latest Prerelease`
- `Publish Version Release`

Expected: Windows build job uploads artifacts only.

- [ ] **Step 3: Add a new `build-macos-arm64` job**

Add a second job with:
- `runs-on: macos-14`
- checkout
- Python 3.10 setup
- dependency install into `.venv`
- `python -m unittest discover -s tests -v`
- `bash ./build_macos.sh`

Expected: CI now has a dedicated macOS packaging path independent of Windows.

- [ ] **Step 4: Add a macOS smoke test step**

Use a bash step shaped like this:

```bash
./dist/ShenYin.app/Contents/MacOS/ShenYin --host 127.0.0.1 --port 19876 --no-browser &
APP_PID=$!
trap 'kill "$APP_PID" || true' EXIT
sleep 5
curl --fail --silent http://127.0.0.1:19876/ > /dev/null
```

Expected:
- launches packaged app binary directly
- binds to the explicit host/port from the spec
- does not open a browser
- fails the job if the root page is not reachable

- [ ] **Step 5: Add a macOS archive step that zips `ShenYin.app` without wrapping it in an extra directory**

Use a command in the repo root such as:

```bash
cd dist && ditto -c -k --sequesterRsrc --keepParent ShenYin.app ShenYin-macos-arm64.zip
```

Expected:
- output file path: `dist/ShenYin-macos-arm64.zip`
- archive expands directly to `ShenYin.app`

- [ ] **Step 6: Upload the macOS workflow artifact**

Add `actions/upload-artifact@v4` with:
- name: `ShenYin-macos-arm64-${{ github.ref_name }}`
- path: `dist/ShenYin-macos-arm64.zip`

Expected: macOS build uploads exactly one zip artifact.

- [ ] **Step 7: Add a required `release` job that depends on both build jobs**

The `release` job should:
- `needs: [build-windows, build-macos-arm64]`
- only run on `main` and `v*` tags, matching current behavior
- download the Windows artifact and macOS artifact
- publish both files together

Expected: release publication is centralized and only happens after both builds succeed.

- [ ] **Step 8: Recreate the current prerelease flow in the `release` job**

Move the current `main` branch logic into the release job:
- move `latest` tag
- publish/update prerelease
- attach both `ShenYin.exe` and `ShenYin-macos-arm64.zip`

Expected: pushes to `main` still update the rolling prerelease, now with both assets.

- [ ] **Step 9: Recreate the current tagged release flow in the `release` job**

Move the current tag logic into the release job:
- publish version release for `v*`
- attach both `ShenYin.exe` and `ShenYin-macos-arm64.zip`

Expected: version tags publish both platform assets in one release.

- [ ] **Step 10: Review the full workflow YAML for path correctness on both shells**

Check specifically:
- Windows uses backslash paths only inside PowerShell/cmd where already established
- macOS uses POSIX paths
- artifact download paths match the uploaded filenames
- release step points at the actual downloaded files

Expected: no broken artifact path assumptions remain.

- [ ] **Step 11: Commit the workflow changes**

```bash
git add .github/workflows/build-and-release.yml
git commit -m "ci: add macOS build and unified release job"
```

## Task 3: Protect compatibility expectations with targeted test coverage

**Files:**
- Modify: `tests/test_crypto.py` (only if needed)
- Test: `tests/test_crypto.py`

- [ ] **Step 1: Review the existing crypto tests against the spec’s compatibility guarantee**

Confirm existing tests already cover:
- text round-trip
- file round-trip
- armored text transport
- wrong-passphrase failure
- compression variants

Expected: most compatibility coverage already exists.

- [ ] **Step 2: If a gap exists, add one minimal failing test that strengthens compatibility protection without introducing platform-specific complexity**

Example candidate only if needed:

```python
def test_binary_ciphertext_roundtrip_preserves_filename_and_bytes(self):
    original = b"cross-platform payload"
    encrypted = encrypt_content(
        input_type="file",
        armor=False,
        compression_name="none",
        passphrase="secret",
        file_name="cross.bin",
        file_bytes=original,
    )

    decrypted = decrypt_content(
        encrypted_blob=encrypted.content,
        passphrase="secret",
    )

    self.assertEqual(decrypted.filename, "cross.bin")
    self.assertEqual(decrypted.content, original)
```

Expected: only add this if existing tests are insufficient.

- [ ] **Step 3: Run the focused crypto test module**

Run:
```bash
python -m unittest tests.test_crypto -v
```

Expected: all tests pass.

- [ ] **Step 4: Commit the test change only if a test was actually added or modified**

```bash
git add tests/test_crypto.py
git commit -m "test: protect cross-platform crypto compatibility"
```

If no test change was needed, skip this commit.

## Task 4: Update user-facing release documentation

**Files:**
- Modify: `README.md`
- Test: `README.md`

- [ ] **Step 1: Update the download section to mention both release assets**

Change the current Windows-only wording so it tells users to download:
- `ShenYin.exe` on Windows
- `ShenYin-macos-arm64.zip` on macOS

Expected: README no longer implies Windows is the only packaged platform.

- [ ] **Step 2: Add a concise macOS note for Apple Silicon and unsigned first launch**

Add wording equivalent to:

```markdown
- macOS 版本适用于 Apple Silicon（M1 及更新机型）
- macOS 应用当前未签名，首次打开时可能需要在系统安全设置中手动允许
```

Expected: users know the target hardware and first-run behavior.

- [ ] **Step 3: Keep the release trigger explanation aligned with the workflow**

Ensure README still states:
- pushes to `main` produce the rolling prerelease
- `v*` tags produce formal releases
- both flows now include Windows and macOS assets

Expected: docs match the implemented CI behavior exactly.

- [ ] **Step 4: Commit the README changes**

```bash
git add README.md
git commit -m "docs: add macOS release instructions"
```

## Task 5: Validate the final change set

**Files:**
- Modify: none
- Test: `.github/workflows/build-and-release.yml`, `README.md`, `build_macos.sh`, `tests/test_crypto.py`

- [ ] **Step 1: Run the full local test suite**

Run:
```bash
python -m unittest discover -s tests -v
```

Expected: PASS for all tests.

- [ ] **Step 2: Review the final diff for scope control**

Check:
- no crypto protocol changes
- no app/UI feature changes
- workflow includes required `release` job
- macOS archive name is `ShenYin-macos-arm64.zip`

Expected: diff matches the approved spec and nothing more.

- [ ] **Step 3: Review git status for only intended files**

Run:
```bash
git status --short
```

Expected: only workflow/docs/build-script/test files are modified or added.

- [ ] **Step 4: Create a final integration commit**

```bash
git add .github/workflows/build-and-release.yml build_macos.sh README.md tests/test_crypto.py local-workspace.spec
git commit -m "feat: add macOS ARM64 release build"
```

If `tests/test_crypto.py` or `local-workspace.spec` did not change, omit them from `git add`.

- [ ] **Step 5: Push and verify CI on GitHub**

Run:
```bash
git push origin <branch>
```

Then verify:
- `build-windows` succeeds
- `build-macos-arm64` succeeds
- `release` succeeds when run on `main` or a version tag
- release assets contain both files

Expected: GitHub produces a Windows executable and a macOS ARM64 zip from the same codebase.
