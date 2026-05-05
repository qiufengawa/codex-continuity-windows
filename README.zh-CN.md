# Codex Continuity for Windows

[English README](README.md)

Codex Continuity for Windows 是一个本地 GUI 工具，用来解决切换 Codex CLI API Provider 后，本地历史会话难以继续 `codex resume` 的问题。

这个 Windows 版使用 PowerShell + WPF，因此标准 Windows 环境即可运行，不需要 Python，也不需要安装 .NET SDK。

## 系统要求

- Windows 10 或 Windows 11
- Windows PowerShell 5.1 或更高版本
- Codex CLI 本地数据目录：`%USERPROFILE%\.codex`

## 运行

双击：

```text
Codex Continuity Windows.bat
```

如果 PowerShell 执行策略阻止脚本，启动器会对当前进程使用 `-ExecutionPolicy Bypass`。

## 功能

- 读取 `%USERPROFILE%\.codex\sessions` 下的本地 Codex 会话。
- 可选包含归档会话。
- 会话预览和详情查看。
- 生成原生 `codex resume` 命令。
- 诊断 `/resume` 风险。
- Provider 同步，并自动备份。
- 导出/复制轻量恢复提示词。
- 不依赖 Python。

## 安全说明

App 会读取本地 Codex JSONL 日志。只有当你明确点击 **Sync to Current Provider** 或导出恢复提示词时才会写文件。Provider 同步前会在 `%USERPROFILE%\.codex` 下创建带时间戳的备份目录。

## 包内容

```text
CodexContinuityWindows.ps1
Codex Continuity Windows.bat
assets/logo.png
README.md
README.zh-CN.md
```

## License

MIT
