# Local PGP Workbench

一个纯 Python 的本地 PGP 工具：

- 不依赖系统安装的 `gpg` / `gpg.exe`
- 支持对称加密 / 公钥加密
- 支持 ASCII Armored `.asc` 和二进制 `.pgp`
- 支持纯文本和任意文件，包括压缩包
- 支持 PGP 压缩选项：`ZLIB` / `ZIP` / `BZ2`
- 支持本地生成 / 导入 / 下载 / 删除密钥
- 默认在浏览器打开本地页面，离线可用

## 运行

```powershell
python -m venv .venv
.\.venv\Scripts\python -m pip install -r requirements.txt
.\.venv\Scripts\python app.py
```

启动后会自动打开：

```text
http://127.0.0.1:8765
```

## 使用建议

- 想最省事：直接用“口令加密 / 口令解密”
- 想多设备或多人交换：先在“密钥管理”里生成或导入密钥，再用“公钥加密 / 私钥解密”
- 纯文本传输场景：保持“文本输入”，工具会默认输出可复制粘贴的 `ASCII Armor`
- 大文本传输场景：工具会自动改成分段复制，解密端支持多段连续粘贴后自动合并
- 想减小输出体积：优先选 `Binary .pgp`
- 想方便复制粘贴：选 `ASCII Armor .asc`

## 说明

- `requirements.txt` 只保留运行时依赖，不包含打包工具。
- 结果下载文件会暂存在 `data/results`，默认保留 24 小时。
- 密钥库保存在 `data/keys`。
