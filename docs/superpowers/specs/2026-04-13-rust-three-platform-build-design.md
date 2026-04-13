# 2026-04-13 ShenYin Rust three-platform build design

## Summary

将当前基于 Python + PyInstaller 的双平台发布链，迁移为基于 Rust + Cargo 的三平台 GitHub Actions 发布链。构建仅在 GitHub Actions 中完成，目标产物为 Windows x64 `ShenYin.exe`、Linux x64 `ShenYin-linux-x64.tar.gz`、macOS arm64 `ShenYin-macos-arm64.zip`。发布仍保持单独 `release` job 聚合 artifact，避免并发写 release。

## Goals

- 用 Rust/Cargo 替换当前 Python/PyInstaller 构建链
- 支持 Windows x64、Linux x64、macOS arm64 三平台发布
- 保留当前 `main` -> rolling prerelease、`v*` -> 正式 release 的模型
- 保留 headless smoke test 契约：启动产物、监听 `127.0.0.1:19876`、`--no-browser`、验证 `/` 返回 200
- 保留现有加密协议与文本传输格式，避免跨版本/跨平台不兼容

## Non-goals

- 不处理本地三平台交叉编译能力
- 不引入代码签名、公证或 notarization
- 不扩展到 Windows arm64、Linux arm64、Intel macOS
- 不改变 `/api/*` 语义、加密协议、版本 framing 或文本传输编码规则

## Current state

### Release pipeline

仓库当前 workflow 在 `.github/workflows/build-and-release.yml`，现状是：

- `build-windows`：`windows-latest`，Python 3.10，运行 `build_exe.bat`
- `build-macos-arm64`：`macos-14`，Python 3.10，运行 `build_macos.sh`
- `release`：在 `ubuntu-latest` 下载两个 artifact 并统一发布

这说明仓库已经具备可复用的统一发版骨架，但构建本体仍是 Python，不包含 Linux build job，也不包含 Rust toolchain、Cargo build、Rust smoke test 路径。

### Existing compatibility contract

Rust 迁移必须保留以下外部可见契约：

- CLI 启动参数：`--host`、`--port`、`--no-browser`（来自 `app.py`）
- Smoke test 最小 HTTP 契约：访问 `/` 返回 200（来自 `tests/test_app.py` 与 workflow）
- 用户下载习惯：Windows 下载单文件 exe，macOS 下载 zip 包裹的 `.app`
- Release 聚合模式：build jobs 只上传 artifact，release job 统一发布

### Crypto and transport invariants

现有协议来自 `pgpbox/engine.py`、`pgpbox/crypto.py`、`pgpbox/transport.py`，第一阶段 Rust 版不能改这些约定：

- AES-256-GCM
- PBKDF2-HMAC-SHA256，600,000 次迭代
- version byte + salt + nonce + metadata length + metadata JSON + ciphertext 的 payload framing
- 压缩选项：`zlib` / `zip` / `bz2` / `none`
- 文本模式使用 Python `base64.b85encode` 语义
- 文本输出按 76 列换行
- 超长文本阈值：`100_000`

其中 Base85 字母表已核实为：

```text
0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~
```

Rust 版若不兼容该编码语义，现有文本模式数据将无法无损互通。

## Recommended approach

采用：

1. Rust 主程序先复刻最小可启动 HTTP 契约
2. 平台打包逻辑下沉到独立脚本
3. GitHub Actions 改为单一 matrix build + 独立 release job
4. README 与 release body 同步迁移到 Rust 三平台叙事

## Design

### 1. Rust application baseline

第一阶段 Rust 程序至少需要提供：

- 命令行参数：`--host`、`--port`、`--no-browser`
- 本地 HTTP 服务
- `/` 首页返回 200
- 端口占用时明确失败退出
- `--no-browser` 时不自动打开浏览器

在完整加密 UI/接口迁移前，这个最小基线已经足够支撑 CI smoke test 与 release workflow 改造。

### 2. Packaging outputs

目标产物：

- Windows: `ShenYin.exe`
- Linux: `ShenYin-linux-x64.tar.gz`
- macOS: `ShenYin-macos-arm64.zip`（内部为 `ShenYin.app`）

打包脚本建议：

- `scripts/package-windows.ps1`
- `scripts/package-linux.sh`
- `scripts/package-macos-app.sh`

### 3. Workflow structure

推荐改造成单一 `build` matrix job：

- `windows-latest` / `x86_64-pc-windows-msvc`
- `ubuntu-latest` / `x86_64-unknown-linux-gnu`
- `macos-14` / `aarch64-apple-darwin`

每个平台执行：

- checkout
- setup Rust toolchain
- restore Rust cache
- `cargo test`
- `cargo build --release --target ...`
- 平台打包脚本
- smoke test
- upload artifact

随后由 `release` job：

- 下载全部 artifact
- `main` 更新 `latest` prerelease
- `v*` tag 发布正式 release

### 4. Smoke-test contract

每个平台构建后都需要用打包产物执行：

```text
--host 127.0.0.1 --port 19876 --no-browser
```

并轮询：

```text
http://127.0.0.1:19876/
```

直到成功返回 HTTP 200 或超时失败。

### 5. Documentation contract

`README.md` 和 release body 需要同步改成：

- 三平台下载说明
- Linux 新增支持说明
- macOS 未签名提醒继续保留
- 开发叙事从 Python 切到 Rust/Cargo

## Risks

### Risk: workflow 先迁移，应用契约没跟上

**Mitigation:** 先建立 Rust 最小服务与 smoke contract，再改 workflow。

### Risk: Rust 版改坏现有加密/传输兼容性

**Mitigation:** 迁移阶段把协议与文本传输视为硬约束，先做兼容性回归测试，再替换完整应用逻辑。

### Risk: macOS `.app` 打包工具细节过早锁死

**Mitigation:** 先锁定产物形态与 smoke test 契约，不在 design 阶段绑定具体工具。

### Risk: release 聚合文件名错配

**Mitigation:** 为 artifact 名称、release files、README 下载说明分别加回归测试。

## Execution snapshot

截至 2026-04-13 当前已确认：

- 当前 workflow 仍是 Python 双平台，不含 Linux build
- 统一 `release` job 骨架可复用
- 本地是否具备 Linux cross build 能力不再是阻塞项，因为构建全部放 GitHub Actions
- `app.py` 的 CLI 契约与 `/` 200 smoke contract 已识别
- `pgpbox/engine.py` 的协议 framing 与 `pgpbox/transport.py` 的 Base85/76 列换行约束已识别
- Python `b85encode` 使用的字母表已实测确认，后续 Rust 实现必须对齐

当前正在推进：

- 把这些契约写入执行文档
- 准备落地 Unit 1 的 Rust 工程入口与最小 smoke contract
- 随后再进入三平台打包脚本与 workflow 改造

## Expected files to change

- `Cargo.toml`
- `rust-toolchain.toml`
- `src/main.rs`
- `tests/smoke_contract.rs`
- `.github/workflows/build-and-release.yml`
- `README.md`
- `scripts/package-windows.ps1`
- `scripts/package-linux.sh`
- `scripts/package-macos-app.sh`
- `packaging/macos/Info.plist`

## References

- `.github/workflows/build-and-release.yml`
- `app.py`
- `tests/test_app.py`
- `pgpbox/engine.py`
- `pgpbox/crypto.py`
- `pgpbox/transport.py`
- `docs/plans/2026-04-13-001-feat-rust-release-workflow-plan.md`
- `docs/superpowers/specs/2026-03-30-macos-arm64-build-design.md`
