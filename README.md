# ShenYin

本地数据整理工具，纯离线运行。

## 功能

- 支持纯文本和任意文件的双向处理（整理 / 提取）
- 访问码模式：输入访问码即可加密解密，简单直接
- 文本输出使用 Base85 高效编码，无外部格式标记
- 支持多种压缩等级（标准 / 兼容 / 高压缩 / 不压缩）
- 纯 Python 加密引擎（AES-256-GCM + PBKDF2），无外部依赖
- 默认打开本地浏览器页面，所有数据仅在本机流转

## 下载使用

从 [Releases](https://github.com/realnghon/ShenYin/releases) 下载：

- Windows：`ShenYin.exe`
- macOS（Apple Silicon）：`ShenYin-macos-arm64.zip`

首次在 macOS 上打开可能提示“已损坏”或“无法验证开发者”，在“系统设置 → 隐私与安全性”中允许后即可运行。

- 无需安装 Python 或任何运行环境
- 无需管理员权限
- 所有文件仅在 EXE 所在目录下生成

## 开发运行

```powershell
python -m venv .venv
.\.venv\Scripts\python -m pip install -r requirements.txt
.\.venv\Scripts\python app.py
```

启动后自动打开：

```text
http://127.0.0.1:8765
```

## 使用建议

- 文本传输：保持文本格式输出，结果可直接复制粘贴
- 文件传输：选择文件格式输出，体积更小
- 超长文本：可直接粘贴，工具会自动进入缓存模式处理

## 技术细节

- 加密：AES-256-GCM 认证加密
- 密钥派生：PBKDF2-HMAC-SHA256，600,000 次迭代
- 每次加密使用随机 16 字节盐值 + 12 字节随机数
- 传输编码：Base85（文本模式）

## 发布

- 推送到 `main` 分支会自动构建并发布滚动预发布版
- 打 `v*` 标签（如 `v2.0.0`）会发布正式版本

## Contributors

- [realnghon](https://github.com/realnghon)
- [Claude](https://claude.ai) — Anthropic AI assistant
