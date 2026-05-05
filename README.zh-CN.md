# Codex Continuity for Windows

[English README](README.md)

Codex Continuity for Windows 是一个原生 Windows GUI 工具，用来解决频繁切换 Codex CLI API Provider 后，本地历史会话难以继续 `codex resume` 的问题。界面默认中文，并内置中文/英文切换。

发布包里是真正可双击运行的 `.exe`。最终用户不需要安装 Python、PowerShell 脚本、Rust、.NET SDK，也不需要任何开发环境。

## 系统要求

- Windows 10 或 Windows 11，64 位 x86 电脑
- 默认中文界面；点击 App 内的 **English** 按钮可切换英文
- Codex CLI 本地数据目录：`%USERPROFILE%\.codex`

## 运行方式

1. 从 Release 页面下载 `CodexContinuityWindows.exe.zip`。
2. 解压 zip。
3. 双击：

```text
Codex Continuity.exe
```

## 功能

- 读取 `%USERPROFILE%\.codex\sessions` 下的本地 Codex 会话。
- 可选包含归档会话。
- 搜索、会话预览、会话详情查看。
- 生成原生 `codex resume` 命令。
- 诊断 Provider、response storage、API 兼容性等 `/resume` 风险。
- 修改前先预览 Provider 同步内容。
- 将 session/agent 里的 Provider 元数据同步到当前 Provider，并自动备份。
- 导出或复制轻量恢复提示词。
- 打开本地 sessions 文件夹。
- 在中文和英文之间切换界面与用户可见消息。

## 安全说明

App 会读取本地 Codex JSONL 日志。只有当你明确点击 **同步到当前 Provider**（英文界面为 **Sync to Current Provider**）或导出恢复文件时才会写文件。

Provider 同步前，会在 `%USERPROFILE%\.codex` 下创建带时间戳的备份目录。

发布包不包含密钥、本地测试数据、构建缓存、`.omx` 状态或临时文件。

## 包内容

```text
Codex Continuity.exe
README.md
README.zh-CN.md
LICENSE
```

## 从源码构建

源码构建使用 Rust 和 `windows` crate。最终用户不需要这些。

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

## License

MIT
