# Codex Continuity for Windows

[中文说明](README.zh-CN.md)

Codex Continuity for Windows is a local GUI utility for keeping Codex CLI sessions usable after switching API providers.

This Windows edition is delivered as a PowerShell + WPF app so it can run on a standard Windows installation without Python or the .NET SDK.

## Requirements

- Windows 10 or Windows 11
- Windows PowerShell 5.1 or later
- Codex CLI local data under `%USERPROFILE%\.codex`

## Run

Double-click:

```text
Codex Continuity Windows.bat
```

If PowerShell execution policy blocks scripts, the launcher uses `-ExecutionPolicy Bypass` for this process only.

## Features

- Lists local Codex sessions from `%USERPROFILE%\.codex\sessions`.
- Optional archived-session scanning.
- Session preview and detailed metadata view.
- Native `codex resume` command generation.
- Resume-risk diagnosis.
- Provider synchronization with backup.
- Lightweight restore prompt export/copy.
- No Python dependency.

## Safety

The app reads local Codex JSONL logs. It only writes files when you explicitly click **Sync to Current Provider** or export a restore prompt. Provider sync creates a timestamped backup folder under `%USERPROFILE%\.codex`.

## Package contents

```text
CodexContinuityWindows.ps1
Codex Continuity Windows.bat
assets/logo.png
README.md
README.zh-CN.md
```

## License

MIT
