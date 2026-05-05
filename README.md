# Codex Continuity for Windows

[中文说明](README.zh-CN.md)

Codex Continuity for Windows is a native Windows GUI utility for keeping Codex CLI local sessions usable after switching API providers.

The release package contains a real double-clickable `.exe`. End users do **not** need Python, PowerShell scripts, Rust, the .NET SDK, or any developer runtime.

## Requirements

- Windows 10 or Windows 11, 64-bit x86 PC
- Codex CLI local data under `%USERPROFILE%\.codex`

## Run

1. Download `CodexContinuityWindows.exe.zip` from the release page.
2. Extract the zip.
3. Double-click:

```text
Codex Continuity.exe
```

## Features

- Lists local Codex sessions from `%USERPROFILE%\.codex\sessions`.
- Optional archived-session scanning.
- Search, session preview, and detailed metadata view.
- Native `codex resume` command generation.
- Resume-risk diagnosis for provider/storage/API mismatch cases.
- Preview provider sync before changing anything.
- Sync session/agent provider metadata to the current provider with automatic backup.
- Export or copy a lightweight restore prompt.
- Open the local sessions folder.

## Safety

The app reads local Codex JSONL logs. It writes only when you explicitly click **Sync to Current Provider** or export a restore file.

Provider sync creates a timestamped backup folder under `%USERPROFILE%\.codex` before modifying session or agent metadata.

No secrets, local test data, build cache, `.omx` state, or temporary files are included in the release package.

## Package contents

```text
Codex Continuity.exe
README.md
README.zh-CN.md
LICENSE
```

## Build from source

Source builds use Rust and the `windows` crate. End users do not need this.

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

## License

MIT
