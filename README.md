# ShenYin

ShenYin 正在从 Python/PyInstaller 迁移到 Rust/Cargo。

当前主分支的 Rust 版本还只是启动、打包和发布链路基线，**还没有完成加密/解密功能迁移**。所以它目前只能验证本地 HTTP 服务和三平台构建，不适合作为最终用户下载使用。

如果你现在需要可用版本，请先使用稳定正式版：

- [v2.1.1 release](https://github.com/realnghon/ShenYin/releases/tag/v2.1.1)
- [Windows `ShenYin.exe`](https://github.com/realnghon/ShenYin/releases/download/v2.1.1/ShenYin.exe)
- [macOS `ShenYin-macos-arm64.zip`](https://github.com/realnghon/ShenYin/releases/download/v2.1.1/ShenYin-macos-arm64.zip)

## 当前 Rust 基线

- 保留启动参数契约：`--host`、`--port`、`--no-browser`
- 默认启动本地浏览器，也支持无界面 smoke test
- 默认双击启动时，如果 `127.0.0.1:8765` 上已经是 ShenYin，会直接复用已有实例
- `GET /` 返回 HTTP 200，供 CI 和手工验证使用
- 现有 Python 协议实现仍保留在仓库中，后续会继续迁移到 Rust，当前阶段不改加密和传输约束
- `main` 只做三平台构建验证；只有 `v*` tag 才会发布正式 release

## 下载使用

Rust 基线构建的目标产物仍然是：

- Windows：`ShenYin.exe`
- Linux：`ShenYin-linux-x64.tar.gz`
- macOS（Apple Silicon）：`ShenYin-macos-arm64.zip`

macOS 应用仍然是未签名 `.app`，首次打开可能需要在“系统设置 -> 隐私与安全性”中手动允许。

### 启动方式

- 直接双击 Windows EXE 或 macOS `.app`
- Linux 解压后运行 `./ShenYin`
- 命令行模式可显式传参：`--host 127.0.0.1 --port 8765 --no-browser`

## 本地开发

```powershell
cargo run -- --host 127.0.0.1 --port 8765
```

如果只想验证 CI 使用的最小契约：

```powershell
cargo test
```

## 迁移说明

- 当前 Rust 代码的重点是验证启动、打包和 GitHub Actions 链路
- 加密/解密 UI 与协议逻辑还没有迁移完成，所以现在还不能替代稳定版
- 历史 Python 代码和协议实现仍保留在 `app.py`、`pgpbox/`、`tests/`，作为迁移参考
- 后续阶段会在不破坏既有协议的前提下，把核心功能逐步迁移到 Rust

## 发布

- 推送到 `main` 会自动跑三平台构建与 smoke test
- 打 `v*` 标签（例如 `v2.2.0`）才会自动发布正式版

## Contributors

- [realnghon](https://github.com/realnghon)
- [Claude](https://claude.ai) — Anthropic AI assistant
