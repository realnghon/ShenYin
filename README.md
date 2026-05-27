# FoldBox

FoldBox 是一个纯 Rust CLI 工具，用来把任意文件封装成 `.box.txt` 文本文件，再按原文件名还原回来。

## 命令

```text
foldbox pack <input-file> [output-file]
foldbox unpack <input-file> [output-file]
foldbox --help
```

`pack` 默认输出同目录 `原文件名.box.txt`。  
`unpack` 默认恢复成原始文件名。

## 发布

- `main` 分支只做构建和测试
- `v*` 标签发布正式产物
- 产物包含 Windows x64、Linux x64、macOS arm64 三个平台
