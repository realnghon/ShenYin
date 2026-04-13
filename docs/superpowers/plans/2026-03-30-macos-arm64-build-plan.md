# ShenYin macOS ARM64 Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a macOS ARM64 GitHub Actions build that produces a zipped unsigned `.app`, smoke-tests it headlessly, and publishes it together with the existing Windows `.exe` without changing the encryption protocol.

**Architecture:** Keep the current Windows packaging flow intact, add a separate macOS build entrypoint and runner-specific smoke test, and restructure release publication into a dedicated `release` job that consumes artifacts from both build jobs. Guard the declarative packaging and workflow changes with small `unittest` checks that assert the expected spec, workflow, and README contract so future edits do not silently drop macOS support.

**Tech Stack:** GitHub Actions, Python 3.10, `unittest`, PyInstaller, Windows batch, POSIX shell

---

## File structure

### Files to create
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/build_macos.sh` — macOS-only PyInstaller entrypoint that mirrors `build_exe.bat` responsibilities with POSIX shell semantics.
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_packaging.py` — regression tests for PyInstaller spec shape and build helper scripts.
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_release_workflow.py` — regression tests for workflow job layout, artifact naming, smoke-test command shape, and release-job publication rules.
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_readme_release_notes.py` — regression tests for the user-facing dual-platform download and unsigned macOS messaging.

### Files to modify
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/local-workspace.spec` — keep shared PyInstaller configuration as the source of truth while making the macOS `.app` output explicit if the current `EXE(...)`-only shape is insufficient.
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/.github/workflows/build-and-release.yml` — split the current single Windows job into `build-windows`, `build-macos-arm64`, and `release`.
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/README.md` — document Windows and macOS downloads, Apple Silicon scope, and unsigned-first-launch behavior.

### Files expected not to change
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/pgpbox/engine.py`
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/pgpbox/crypto.py`
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/app.py`
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_crypto.py`
- `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_app.py`

### Constraints to preserve while implementing
- Do not convert the workflow to a matrix build.
- The `release` job is required; build jobs upload artifacts only.
- The macOS smoke test must launch `dist/ShenYin.app/Contents/MacOS/ShenYin` with `--host 127.0.0.1 --port 19876 --no-browser` and require HTTP 200 from `/`.
- Preserve the current encryption payload format and Base85 transport behavior exactly.
- Keep the README changes small and user-facing.

### Task 1: Add packaging regression coverage and a macOS build entrypoint

**Files:**
- Create: `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_packaging.py`
- Create: `C:/Nghon/Develop/Baby/AAAI/ShenYin/build_macos.sh`
- Modify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/local-workspace.spec`
- Verify against existing: `C:/Nghon/Develop/Baby/AAAI/ShenYin/build_exe.bat`

- [ ] **Step 1: Write the failing packaging regression test**

```python
from __future__ import annotations

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PackagingConfigTests(unittest.TestCase):
    def test_macos_build_script_uses_shared_spec(self):
        script = (ROOT / "build_macos.sh").read_text(encoding="utf-8")
        self.assertIn(".venv/bin/python", script)
        self.assertIn("pyinstaller", script)
        self.assertIn("local-workspace.spec", script)

    def test_pyinstaller_spec_defines_shenyin_targets(self):
        spec = (ROOT / "local-workspace.spec").read_text(encoding="utf-8")
        self.assertIn("Analysis(", spec)
        self.assertIn("('templates', 'templates')", spec)
        self.assertIn("('static', 'static')", spec)
        self.assertIn("name='ShenYin'", spec)
        self.assertRegex(spec, r"BUNDLE\(|COLLECT\(")
```

- [ ] **Step 2: Run the targeted test to confirm the current repo is missing the macOS packaging contract**

Run:
```bash
python -m unittest tests.test_packaging.PackagingConfigTests -v
```

Expected: FAIL because `build_macos.sh` does not exist yet and `local-workspace.spec` does not yet satisfy the app-bundle expectation.

- [ ] **Step 3: Implement the minimal packaging changes**

```bash
# build_macos.sh responsibilities
set -euo pipefail

if [ ! -x ".venv/bin/python" ]; then
  echo "Please create the venv first."
  exit 1
fi

".venv/bin/python" -m pip install pyinstaller
".venv/bin/pyinstaller" --noconfirm --clean local-workspace.spec
```

```python
# local-workspace.spec shape to add only if needed for macOS app output
coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=True,
    name='ShenYin',
)

app = BUNDLE(
    coll,
    name='ShenYin.app',
    icon=None,
    bundle_identifier=None,
)
```

Implementation notes:
- Keep `Analysis(...)` resource declarations unchanged.
- Keep the app entrypoint as `app.py`.
- Do not introduce platform branching into `build_exe.bat`.
- If PyInstaller on macOS already emits the `.app` with a smaller spec change, use that smaller change; the plan goal is explicit bundle support, not speculative refactoring.

- [ ] **Step 4: Re-run the targeted packaging regression test**

Run:
```bash
python -m unittest tests.test_packaging.PackagingConfigTests -v
```

Expected: PASS.

- [ ] **Step 5: Verify the build helper contract locally where possible**

Run on Windows host:
```bash
python -m unittest tests.test_packaging.PackagingConfigTests -v
```

Run on a macOS shell or CI runner when available:
```bash
bash ./build_macos.sh
```

Expected on macOS: `dist/ShenYin.app` exists after PyInstaller completes.

- [ ] **Step 6: Commit the packaging groundwork**

```bash
git add tests/test_packaging.py build_macos.sh local-workspace.spec
git commit -m "build: add macOS packaging entrypoint"
```

### Task 2: Split CI into Windows build, macOS build, and unified release publication

**Files:**
- Create: `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_release_workflow.py`
- Modify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/.github/workflows/build-and-release.yml`
- Verify against existing: `C:/Nghon/Develop/Baby/AAAI/ShenYin/build_exe.bat`
- Verify against new helper: `C:/Nghon/Develop/Baby/AAAI/ShenYin/build_macos.sh`

- [ ] **Step 1: Write the failing workflow regression test**

```python
from __future__ import annotations

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ReleaseWorkflowTests(unittest.TestCase):
    def test_workflow_contains_three_required_jobs(self):
        workflow = (ROOT / ".github/workflows/build-and-release.yml").read_text(encoding="utf-8")
        self.assertIn("build-windows:", workflow)
        self.assertIn("build-macos-arm64:", workflow)
        self.assertIn("release:", workflow)

    def test_workflow_contains_required_macos_smoke_test_shape(self):
        workflow = (ROOT / ".github/workflows/build-and-release.yml").read_text(encoding="utf-8")
        self.assertIn("dist/ShenYin.app/Contents/MacOS/ShenYin", workflow)
        self.assertIn("--host", workflow)
        self.assertIn("127.0.0.1", workflow)
        self.assertIn("19876", workflow)
        self.assertIn("--no-browser", workflow)

    def test_release_job_publishes_both_assets(self):
        workflow = (ROOT / ".github/workflows/build-and-release.yml").read_text(encoding="utf-8")
        self.assertIn("ShenYin-win-x64-${{ github.ref_name }}", workflow)
        self.assertIn("ShenYin-macos-arm64-${{ github.ref_name }}", workflow)
        self.assertIn("ShenYin.exe", workflow)
        self.assertIn("ShenYin-macos-arm64.zip", workflow)
```

- [ ] **Step 2: Run the targeted workflow test and confirm it fails before the workflow is restructured**

Run:
```bash
python -m unittest tests.test_release_workflow.ReleaseWorkflowTests -v
```

Expected: FAIL because the current workflow has only one Windows job and no macOS artifact/release logic.

- [ ] **Step 3: Implement the minimal workflow restructure**

```yaml
jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      # checkout, setup-python, install, unittest, build_exe.bat, smoke test, upload ShenYin.exe

  build-macos-arm64:
    runs-on: macos-14
    steps:
      # checkout, setup-python, install, unittest, bash ./build_macos.sh,
      # launch dist/ShenYin.app/Contents/MacOS/ShenYin --host 127.0.0.1 --port 19876 --no-browser,
      # verify GET / == 200, zip ShenYin.app into ShenYin-macos-arm64.zip, upload artifact

  release:
    needs: [build-windows, build-macos-arm64]
    steps:
      # download both artifacts
      # publish latest prerelease on main with both files
      # publish tagged release on v* with both files
```

Implementation notes:
- Rename the current `build-release` job to `build-windows` rather than rewriting its logic from scratch.
- Preserve the existing `python -m unittest discover -s tests -v` command in both build jobs.
- Keep Windows smoke test behavior unchanged.
- Move `softprops/action-gh-release@v2` usage into `release` so only one job updates a release.
- Keep latest-tag movement in the release path, not in either build job.
- Ensure the release job fails if either expected artifact is missing after download.
- On `main`, continue publishing the rolling prerelease and attach both assets if both builds succeed.
- On `v*` tags, publish a single release with both assets attached.

- [ ] **Step 4: Re-run the targeted workflow regression test**

Run:
```bash
python -m unittest tests.test_release_workflow.ReleaseWorkflowTests -v
```

Expected: PASS.

- [ ] **Step 5: Run the full unit test suite to confirm the workflow-specific tests did not disturb app or crypto coverage**

Run:
```bash
python -m unittest discover -s tests -v
```

Expected: PASS, including `tests.test_app`, `tests.test_crypto`, `tests.test_packaging`, and `tests.test_release_workflow`.

- [ ] **Step 6: Commit the CI restructuring**

```bash
git add tests/test_release_workflow.py .github/workflows/build-and-release.yml
git commit -m "ci: add macOS build and unified release job"
```

### Task 3: Document dual-platform downloads and unsigned macOS behavior

**Files:**
- Create: `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_readme_release_notes.py`
- Modify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/README.md`

- [ ] **Step 1: Write the failing README regression test**

```python
from __future__ import annotations

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ReadmeReleaseNotesTests(unittest.TestCase):
    def test_readme_mentions_both_release_downloads(self):
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("ShenYin.exe", readme)
        self.assertIn("ShenYin-macos-arm64.zip", readme)
        self.assertIn("Apple Silicon", readme)
        self.assertRegex(readme, r"unsigned|未签名")
```

- [ ] **Step 2: Run the targeted README test to confirm the current docs are Windows-only**

Run:
```bash
python -m unittest tests.test_readme_release_notes.ReadmeReleaseNotesTests -v
```

Expected: FAIL because `README.md` currently mentions only the Windows executable.

- [ ] **Step 3: Update README with the smallest user-facing doc change that satisfies the spec**

```md
## 下载使用

从 Releases 下载：
- Windows: `ShenYin.exe`
- macOS (Apple Silicon / M1+): `ShenYin-macos-arm64.zip`

macOS 版本未签名，首次打开时可能需要在系统安全设置中手动放行。
```

Implementation notes:
- Keep the existing concise README tone.
- Update the release section so `main` rolling prereleases and `v*` tagged releases clearly imply both assets are published together.
- Do not add notarization instructions or extra troubleshooting beyond the unsigned-first-launch note.

- [ ] **Step 4: Re-run the targeted README regression test**

Run:
```bash
python -m unittest tests.test_readme_release_notes.ReadmeReleaseNotesTests -v
```

Expected: PASS.

- [ ] **Step 5: Re-run the full test suite**

Run:
```bash
python -m unittest discover -s tests -v
```

Expected: PASS.

- [ ] **Step 6: Commit the documentation update**

```bash
git add tests/test_readme_release_notes.py README.md
git commit -m "docs: describe macOS release download"
```

### Task 4: End-to-end verification before handoff

**Files:**
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/.github/workflows/build-and-release.yml`
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/build_macos.sh`
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/local-workspace.spec`
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/README.md`
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_packaging.py`
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_release_workflow.py`
- Verify: `C:/Nghon/Develop/Baby/AAAI/ShenYin/tests/test_readme_release_notes.py`

- [ ] **Step 1: Run the complete local verification set**

Run:
```bash
python -m unittest discover -s tests -v
```

Expected: PASS.

- [ ] **Step 2: Inspect the changed files for scope discipline**

Checklist:
- No edits to `pgpbox/engine.py` or protocol framing.
- No UI or route changes in `app.py`.
- No matrix build conversion.
- Release publication exists only in the dedicated `release` job.
- macOS smoke test launches `dist/ShenYin.app/Contents/MacOS/ShenYin --host 127.0.0.1 --port 19876 --no-browser`.

Expected: All checks satisfied.

- [ ] **Step 3: Verify CI-specific commands are still consistent with each platform**

Manual review targets:
- `build_exe.bat` remains Windows-only.
- `build_macos.sh` remains POSIX-shell-only.
- Artifact names match the spec exactly.
- The release job attaches both `ShenYin.exe` and `ShenYin-macos-arm64.zip`.

Expected: No cross-platform command leakage.

- [ ] **Step 4: Commit any final cleanup only if needed**

```bash
git add .github/workflows/build-and-release.yml build_macos.sh local-workspace.spec README.md tests/test_packaging.py tests/test_release_workflow.py tests/test_readme_release_notes.py
git commit -m "test: lock macOS release packaging contract"
```

Expected: Only create this commit if cleanup remains after the earlier task commits; otherwise skip this step.

---

## Verification commands summary

```bash
python -m unittest tests.test_packaging.PackagingConfigTests -v
python -m unittest tests.test_release_workflow.ReleaseWorkflowTests -v
python -m unittest tests.test_readme_release_notes.ReadmeReleaseNotesTests -v
python -m unittest discover -s tests -v
```

macOS runner verification:

```bash
bash ./build_macos.sh
```

GitHub Actions verification:
- Push a non-tag commit to `main` and confirm the workflow uploads both artifacts, then publishes a single rolling prerelease containing both assets.
- Push a `v*` tag and confirm the tagged release contains both `ShenYin.exe` and `ShenYin-macos-arm64.zip`.

## Review notes for the implementing agent

- Treat the current `local-workspace.spec` as the shared source of truth, but verify the exact PyInstaller object graph needed for `.app` output rather than assuming the current `EXE(...)`-only shape is enough.
- Prefer the smallest viable declarative changes in workflow and spec files.
- Keep every commit scoped to one task so rollback stays easy.
- If the packaging verification reveals that the spec can stay smaller than this plan’s sample snippets, keep the smaller working version and update the regression test to reflect the actual supported contract.
