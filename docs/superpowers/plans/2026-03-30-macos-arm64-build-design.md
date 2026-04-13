# macOS ARM64 GitHub Actions Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不改变现有加密协议的前提下，为 GitHub Actions 增加 Apple Silicon macOS 构建、打包、冒烟测试与双平台统一发布流程。

**Architecture:** 保留现有 Windows 构建链路，新增独立的 `build-macos-arm64` job，并用强制性的 `release` job 统一收集两个平台产物后再发布。继续复用 `local-workspace.spec` 作为 PyInstaller 主配置，只把平台差异收敛到单独的 macOS 构建脚本和 CI 中的 smoke test / zip 打包逻辑。

**Tech Stack:** GitHub Actions, Python 3.10, PyInstaller, Flask, unittest, PowerShell, Bash

---

## File structure

- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\.github\workflows\build-and-release.yml`
  - 将当前单一 Windows 构建发布 job 拆成 `build-windows`、`build-macos-arm64`、`release` 三段。
  - 让两个构建 job 只负责测试、打包、上传 artifact；让 `release` 统一发布 `latest` 和 `v*` tag 版本资产。
- Create: `C:\Nghon\Develop\Baby\AAAI\ShenYin\build_macos.sh`
  - 封装 macOS 下的 PyInstaller 调用与基础前置校验，避免把跨平台分支塞进 `build_exe.bat`。
- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\README.md`
  - 更新 Releases 下载说明、Apple Silicon 适配说明、未签名 app 首次启动提示。
- Optional Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\local-workspace.spec`
  - 仅当 macOS PyInstaller 实测需要极小调整时才改；默认保持共享 spec。
- Optional Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\tests\test_crypto.py`
  - 仅在需要补一个协议不变性保护测试时才改，禁止引入平台特有测试复杂度。

## Planning notes for implementers

- 现有 workflow 只有一个 `build-release` Windows job，且它直接完成 artifact 上传、latest tag 移动、release 发布；计划中要先拆分责任，再保证行为不回退。
- 现有 `build_exe.bat` 只检查 `.venv\Scripts\python.exe` 并调用 `pyinstaller local-workspace.spec`。macOS 版本不应复用这个 bat，而应新增 shell script。
- `app.py` 已支持 `--host`、`--port`、`--no-browser`，适合继续用于 packaged smoke test。
- 现有测试入口是 `python -m unittest discover -s tests -v`，计划默认沿用，不引入 pytest。
- 当前 README 只写 Windows 下载路径，必须改成同时覆盖 Windows 与 macOS。
- 规格明确要求：不能改 `pgpbox/engine.py` 协议格式、transport 编码行为、PBKDF2/AES-GCM framing。

### Task 1: 先补发布流程重构的失败保护测试思路

**Files:**
- Test: `C:\Nghon\Develop\Baby\AAAI\ShenYin\tests\test_crypto.py`
- Reference: `C:\Nghon\Develop\Baby\AAAI\ShenYin\.github\workflows\build-and-release.yml`

- [ ] **Step 1: 判断是否真的需要新增协议保护测试**

检查 `tests/test_crypto.py` 现有覆盖范围，确认它是否已经覆盖文本/文件 round-trip、压缩选项与错误访问码场景。

Decision rule:
- 如果现有测试已足以覆盖“协议未变”的核心事实，则不要新增测试。
- 只有当实现过程中碰到 packaging 相关改动间接触及 crypto 接口时，才新增一个最小 round-trip 保护测试。

- [ ] **Step 2: 若需要，先写失败测试再改实现**

示例最小测试骨架：

```python
    def test_protocol_roundtrip_still_returns_original_text(self):
        encrypted = encrypt_content(
            input_type="text",
            armor=True,
            compression_name="zlib",
            passphrase="secret",
            text_value="cross-platform payload",
        )

        decrypted = decrypt_content(
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.inline_text, "cross-platform payload")
```

- [ ] **Step 3: 运行单测，确认失败或确认无需该测试**

Run: `python -m unittest tests.test_crypto.CryptoTests -v`

Expected:
- 如果新增了测试但尚未实现辅助改动，可能失败。
- 如果没有新增测试，应记录“现有测试已足够，无需额外测试”。

- [ ] **Step 4: 仅在测试确有必要时做最小实现修正**

限制：
- 不得修改 `pgpbox/engine.py` 协议字段、版本字节、framing。
- 不得为了“更稳”而引入与本需求无关的 crypto 重构。

- [ ] **Step 5: 重新运行 crypto 测试**

Run: `python -m unittest tests.test_crypto.CryptoTests -v`
Expected: `OK`

- [ ] **Step 6: Commit**

```bash
git add tests/test_crypto.py
git commit -m "test: preserve crypto roundtrip coverage"
```

### Task 2: 添加 macOS 构建脚本

**Files:**
- Create: `C:\Nghon\Develop\Baby\AAAI\ShenYin\build_macos.sh`
- Reference: `C:\Nghon\Develop\Baby\AAAI\ShenYin\build_exe.bat`
- Reference: `C:\Nghon\Develop\Baby\AAAI\ShenYin\local-workspace.spec`

- [ ] **Step 1: 写一个最小 shell 脚本草案**

目标行为：
- 检查 `.venv/bin/python` 是否存在
- 可选删除旧的 `build/`、`dist/`
- 通过 venv 内的 Python 安装 `pyinstaller`
- 执行 `python -m PyInstaller --noconfirm --clean local-workspace.spec`
- 任一步骤失败即退出

脚本目标内容：

```bash
#!/usr/bin/env bash
set -euo pipefail

if [ ! -x ".venv/bin/python" ]; then
  echo "Please create the venv first."
  exit 1
fi

rm -rf build dist
".venv/bin/python" -m pip install pyinstaller
".venv/bin/python" -m PyInstaller --noconfirm --clean local-workspace.spec
```

- [ ] **Step 2: 赋予脚本可执行权限并检查内容**

Run: `chmod +x build_macos.sh`
Expected: command succeeds with no output

- [ ] **Step 3: 如本地有 macOS 环境则做脚本语法验证，否则只做静态检查**

Run: `bash -n build_macos.sh`
Expected: no output

- [ ] **Step 4: 只在 macOS PyInstaller 失败时再考虑调整 shared spec**

优先级：
1. 先用 shared spec 直接跑
2. 只有失败且问题明确定位到 spec 时，才改 `local-workspace.spec`
3. spec 改动必须保持 Windows 构建不回退

- [ ] **Step 5: Commit**

```bash
git add build_macos.sh
git commit -m "build: add macOS packaging script"
```

### Task 3: 拆分 GitHub Actions 为双构建 job + 统一 release job

**Files:**
- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\.github\workflows\build-and-release.yml`
- Reference: `C:\Nghon\Develop\Baby\AAAI\ShenYin\build_exe.bat`
- Reference: `C:\Nghon\Develop\Baby\AAAI\ShenYin\build_macos.sh`

- [ ] **Step 1: 先把现有单 job 拆成三个 job 骨架**

目标结构：

```yaml
jobs:
  build-windows:
    runs-on: windows-latest
    ...

  build-macos-arm64:
    runs-on: macos-14
    ...

  release:
    needs:
      - build-windows
      - build-macos-arm64
    runs-on: ubuntu-latest
    ...
```

要求：
- `build-windows` 继续负责 checkout、setup-python、安装依赖、测试、build、smoke test、upload artifact
- `build-macos-arm64` 负责对应 macOS 版本动作
- `release` 负责 latest / version release 发布

- [ ] **Step 2: 保持 Windows 构建逻辑可工作，但移除其中的发布逻辑**

把以下步骤从 Windows build job 移走到 `release`：
- `Move Latest Tag`
- `Publish Latest Prerelease`
- `Publish Version Release`

保留：
- `dist/ShenYin.exe` 构建
- EXE smoke test
- `ShenYin-win-x64-${{ github.ref_name }}` artifact 上传

- [ ] **Step 3: 添加 macOS build job 的依赖安装与测试步骤**

建议 YAML 片段：

```yaml
  build-macos-arm64:
    runs-on: macos-14
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: "3.10"
          cache: "pip"

      - name: Install Dependencies
        run: |
          python -m venv .venv
          ./.venv/bin/python -m pip install --upgrade pip
          ./.venv/bin/python -m pip install -r requirements.txt pyinstaller

      - name: Run Tests
        run: |
          ./.venv/bin/python -m unittest discover -s tests -v
```

- [ ] **Step 4: 添加 macOS 打包、冒烟测试、zip 打包步骤**

建议 YAML 片段：

```yaml
      - name: Build app bundle
        run: ./build_macos.sh

      - name: Smoke Test app bundle
        run: |
          APP_PID=""
          cleanup() {
            if [ -n "$APP_PID" ]; then
              kill "$APP_PID" || true
            fi
          }
          trap cleanup EXIT

          dist/ShenYin.app/Contents/MacOS/ShenYin --host 127.0.0.1 --port 19876 --no-browser &
          APP_PID=$!

          python - <<'PY'
import time
import urllib.request

url = "http://127.0.0.1:19876/"
last_error = None
for _ in range(20):
    try:
        with urllib.request.urlopen(url, timeout=2) as response:
            if response.status == 200:
                print("Smoke test passed: HTTP 200")
                raise SystemExit(0)
    except Exception as exc:
        last_error = exc
        time.sleep(1)
raise SystemExit(f"Smoke test failed: {last_error}")
PY

      - name: Zip app bundle
        run: |
          ditto -c -k --sequesterRsrc --keepParent dist/ShenYin.app dist/ShenYin-macos-arm64.zip

      - name: Upload Workflow Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ShenYin-macos-arm64-${{ github.ref_name }}
          path: dist/ShenYin-macos-arm64.zip
          retention-days: 14
```

- [ ] **Step 5: 添加统一 release job，先下载 artifacts 再发布**

建议 job 结构：

```yaml
  release:
    needs:
      - build-windows
      - build-macos-arm64
    runs-on: ubuntu-latest
    steps:
      - name: Download Windows artifact
        uses: actions/download-artifact@v4
        with:
          name: ShenYin-win-x64-${{ github.ref_name }}
          path: release-assets/windows

      - name: Download macOS artifact
        uses: actions/download-artifact@v4
        with:
          name: ShenYin-macos-arm64-${{ github.ref_name }}
          path: release-assets/macos
```

发布前必须确认以下文件存在：
- `release-assets/windows/ShenYin.exe`
- `release-assets/macos/ShenYin-macos-arm64.zip`

- [ ] **Step 6: 在 release job 中实现 latest prerelease 发布**

行为要求：
- 只在 `refs/heads/main` 上执行
- 先移动 `latest` tag，再发布 latest prerelease
- release body 改成同时说明 Windows x64 与 macOS ARM64

建议 body：

```text
Rolling build from `main` (Windows x64 + macOS ARM64).

Commit: `${{ github.sha }}`
```

- [ ] **Step 7: 在 release job 中实现 version release 发布**

行为要求：
- 只在 `refs/tags/v*` 上执行
- 使用 `generate_release_notes: true`
- 同时附带两个文件

- [ ] **Step 8: 本地静态检查 workflow 结构，避免语法错误**

至少逐项核对：
- `needs` 指向的 job 名存在
- artifact 名与 download-artifact 名完全一致
- release job 的 `files` 同时包含两个文件路径
- Windows build job 不再直接发布 release

如果本地装了 actionlint：
Run: `actionlint`
Expected: no errors

如果没有 actionlint：
- 逐段人工审查 YAML 缩进与表达式引用

- [ ] **Step 9: Commit**

```bash
git add .github/workflows/build-and-release.yml build_macos.sh
git commit -m "ci: add macOS ARM64 build and unified release"
```

### Task 4: 仅在必要时做 PyInstaller shared spec 的最小修正

**Files:**
- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\local-workspace.spec`
- Reference: `C:\Nghon\Develop\Baby\AAAI\ShenYin\app.py`

- [ ] **Step 1: 先不修改 spec，等 macOS build 失败后再决定**

当前 spec 已包含：
- `app.py`
- `templates/`
- `static/`
- `console=False`

默认判断：这已经足够生成 macOS `.app`。

- [ ] **Step 2: 如果 macOS CI 失败，定位是否为 spec 问题**

只接受这类证据：
- 缺少资源目录
- app bundle entrypoint 不正确
- macOS 特定打包参数缺失且可最小修复

不接受这类理由：
- “顺手优化一下”
- “统一成更通用架构”
- “未来可能支持 Intel”

- [ ] **Step 3: 若需要修正，只做最小 diff**

示例：仅增加必要的 bundle 配置，不改变 app 行为。

禁止：
- 改 entrypoint
- 改资源目录结构
- 加入与当前需求无关的 hiddenimports/excludes 大清洗

- [ ] **Step 4: 重新验证两个平台构建路径未回退**

需要确认：
- Windows 仍产出 `dist/ShenYin.exe`
- macOS 产出 `dist/ShenYin.app`

- [ ] **Step 5: Commit**

```bash
git add local-workspace.spec
git commit -m "build: adjust PyInstaller spec for macOS packaging"
```

### Task 5: 更新 README 的双平台发布说明

**Files:**
- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\README.md`

- [ ] **Step 1: 先写 README 文案改动草稿**

目标改动点：
- 把“从 Releases 下载 `ShenYin.exe`”改成同时描述 Windows 与 macOS 下载项
- 标明 macOS 仅支持 Apple Silicon（M1 及以上）
- 标明 macOS app 未签名，首次启动可能需要系统安全设置手动放行
- 保持说明简短，不扩写无关安装教程

建议文案：

```md
## 下载使用

从 [Releases](https://github.com/realnghon/ShenYin/releases) 下载对应平台文件：

- Windows：`ShenYin.exe`
- macOS (Apple Silicon)：`ShenYin-macos-arm64.zip`

说明：
- 无需安装 Python 或任何运行环境
- Windows 无需管理员权限
- macOS 版本未签名，首次启动可能需要在系统安全设置中手动允许
```

- [ ] **Step 2: 检查 README 其他段落是否需要最小联动修改**

只允许最小联动：
- `发布` 一节可改成“双平台自动构建并发布”

不要新增：
- 长篇 macOS 安装指南
- 与本需求无关的架构说明

- [ ] **Step 3: 手工检查 Markdown 可读性**

核对：
- 标题层级未破坏
- Releases 链接未变
- 平台命名与 artifact 名一致

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: describe Windows and macOS release assets"
```

### Task 6: 运行验证并确认成功标准

**Files:**
- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\.github\workflows\build-and-release.yml`
- Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\README.md`
- Create: `C:\Nghon\Develop\Baby\AAAI\ShenYin\build_macos.sh`
- Optional Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\local-workspace.spec`
- Optional Modify: `C:\Nghon\Develop\Baby\AAAI\ShenYin\tests\test_crypto.py`

- [ ] **Step 1: 运行 Python 单元测试**

Run: `python -m unittest discover -s tests -v`
Expected: `OK`

- [ ] **Step 2: 本地检查 Windows 构建入口未被破坏**

如果在 Windows 开发机上：
Run: `build_exe.bat`
Expected: 生成 `dist\ShenYin.exe`

如果当前环境无法执行：
- 至少确认 workflow 中 Windows job 仍使用 `build_exe.bat`
- 确认 EXE smoke test 目标仍是 `dist\ShenYin.exe`

- [ ] **Step 3: 如在 macOS 环境可用则本地验证 app bundle；否则依赖 CI 首次验证**

Run: `./build_macos.sh`
Expected: 生成 `dist/ShenYin.app`

Run: `bash -c 'dist/ShenYin.app/Contents/MacOS/ShenYin --host 127.0.0.1 --port 19876 --no-browser'`
Expected: app 启动并可被 `http://127.0.0.1:19876/` 访问

- [ ] **Step 4: 逐项核对成功标准**

Checklist:
- [ ] `main` 推送仍会产出 Windows artifact
- [ ] workflow 也会产出 `ShenYin-macos-arm64.zip`
- [ ] `release` job 统一发布两个资产
- [ ] macOS smoke test 使用 packaged app bundle，而不是源码模式
- [ ] 没有修改 `pgpbox/engine.py`
- [ ] README 已说明 Apple Silicon 与 unsigned app 提示

- [ ] **Step 5: Commit 最终验证后的修正**

```bash
git add .github/workflows/build-and-release.yml README.md build_macos.sh local-workspace.spec tests/test_crypto.py
git commit -m "chore: verify dual-platform release flow"
```

## Implementation guardrails

- 不要把 workflow 改成 matrix build；规格明确要求保持独立 job。
- 不要在未证明必要前改 `local-workspace.spec`。
- 不要改 `app.py` 路由、UI、默认端口或浏览器行为。
- 不要改 `pgpbox/engine.py`、transport 编码、payload 结构。
- 不要把 README 扩写成完整 macOS 使用手册。
- 每个任务结束都应有可验证结果；不要把多个大改堆成一次提交。

## Suggested execution order

1. Task 2 - 先准备 `build_macos.sh`
2. Task 3 - 再拆 workflow 并接入 macOS build/release
3. Task 4 - 仅在 CI 失败时处理 spec
4. Task 5 - 更新 README
5. Task 1 - 仅在实现过程中发现测试覆盖不足时补 crypto 保护测试
6. Task 6 - 最后统一验证
