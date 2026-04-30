# ShenYin

ShenYin 现已切换为纯 Rust 版本，功能覆盖原版的本地加密/解密工作流，并继续通过 GitHub Actions 生成 Windows x64、Linux x64、macOS arm64 三个平台产物。

## 当前功能

- 文本输入与文件输入都支持加密和解密
- 文本模式使用与原版兼容的 Base85 文本传输格式
- 支持 `zlib`、`zip`、`bz2`、`none` 四种压缩选项
- 超长文本结果自动改为下载，不在页面强行展开
- 默认启动本地浏览器，也支持 `--no-browser` 无界面运行
- 默认双击启动时，如果 `127.0.0.1:8765` 上已经是 ShenYin，会直接复用已有实例
- 关闭应用页面后会自动终止本地 API 服务，避免后台继续占用端口
- Windows 发布版不再弹黑框
- 保留启动参数契约：`--host`、`--port`、`--no-browser`

## 下载使用

从 [Releases](https://github.com/realnghon/ShenYin/releases) 下载：

- Windows：`ShenYin-windows-x64.exe`
- Linux：`ShenYin-linux-x64.tar.gz`
- macOS（Apple Silicon）：`ShenYin-macos-arm64.zip`

macOS 产物使用 ad-hoc 签名，不包含 Apple Developer ID 签名或公证；首次打开仍可能需要在“系统设置 -> 隐私与安全性”中手动允许。

### 启动方式

- 直接双击 Windows `ShenYin-windows-x64.exe` 或 macOS `.app`
- Linux 解压后运行 `./ShenYin`
- 命令行模式可显式传参：`--host 127.0.0.1 --port 8765 --no-browser`

## 本地开发

```powershell
cargo run -- --host 127.0.0.1 --port 8765
```

运行测试：

```powershell
cargo test
```

## 兼容性

- 加密协议保持不变：AES-256-GCM + PBKDF2-HMAC-SHA256（600,000 次）
- 文本模式继续兼容原版 Base85 编码和 76 列换行
- 历史数据可继续在 Rust 版中解密

## 发布

- 推送到 `main` 会自动跑三平台构建与 smoke test
- 打 `v*` 标签（例如 `v2.2.0`）会自动发布正式版
- macOS 包面向自用场景；如果系统阻止打开，可右键应用选择“打开”，或在终端执行 `xattr -dr com.apple.quarantine /path/to/ShenYin.app`

## Contributors

- [realnghon](https://github.com/realnghon)
- [Claude](https://claude.ai) — Anthropic AI assistant
