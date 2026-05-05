# Codex Continuity for Windows
# Native Windows PowerShell + WPF GUI. No Python or .NET SDK required.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase
Add-Type -AssemblyName System.Windows.Forms

$Script:CodexHome = Join-Path $env:USERPROFILE '.codex'
$Script:Sessions = @()
$Script:SelectedSession = $null

function ConvertTo-OneLine {
    param([string]$Text, [int]$Limit = 220)
    if ($null -eq $Text) { return '' }
    $compact = (($Text -split '\s+') | Where-Object { $_ }) -join ' '
    if ($compact.Length -le $Limit) { return $compact }
    return $compact.Substring(0, [Math]::Max(0, $Limit - 1)).TrimEnd() + '…'
}

function ConvertTo-CompactTime {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) { return '-' }
    try {
        $dt = [DateTimeOffset]::Parse($Value)
        return $dt.ToLocalTime().ToString('yyyy-MM-dd HH:mm')
    } catch {
        if ($Value.Length -ge 19) { return $Value.Substring(0, 19) }
        return $Value
    }
}

function Get-StringValue {
    param($Value)
    if ($null -eq $Value) { return '' }
    return [string]$Value
}

function Protect-SecretText {
    param([string]$Text)
    if ($null -eq $Text) { return '' }
    $clean = $Text -replace "`0", ''
    $clean = [regex]::Replace($clean, '(?i)(api[_-]?key|token|password|secret|bearer)\s*[:=]\s*[^\s,;]+', '$1=<redacted>')
    $clean = [regex]::Replace($clean, 'sk-[A-Za-z0-9_\-]{16,}', '<redacted>')
    return $clean.Trim()
}

function Test-NoiseMessage {
    param([string]$Text)
    $trimmed = if ($null -eq $Text) { '' } else { $Text.Trim() }
    return $trimmed.StartsWith('<environment_context>') -or $trimmed.StartsWith('# AGENTS.md instructions')
}

function Convert-ContentToText {
    param($Content)
    if ($null -eq $Content) { return '' }
    if ($Content -is [string]) { return $Content }
    if ($Content -is [System.Collections.IEnumerable]) {
        $parts = New-Object System.Collections.Generic.List[string]
        foreach ($item in $Content) {
            if ($item -is [string]) {
                $parts.Add($item)
            } elseif ($null -ne $item) {
                $value = $null
                if ($item.PSObject.Properties['text']) { $value = $item.text }
                elseif ($item.PSObject.Properties['content']) { $value = $item.content }
                elseif ($item.PSObject.Properties['output']) { $value = $item.output }
                if ($null -ne $value) {
                    if ($value -is [string]) { $parts.Add($value) }
                    else { $parts.Add(($value | ConvertTo-Json -Depth 20 -Compress)) }
                }
            }
        }
        return ($parts -join "`n")
    }
    return [string]$Content
}

function Get-SessionIdFromFilename {
    param([string]$Path)
    $name = [IO.Path]::GetFileName($Path)
    $match = [regex]::Match($name, '^rollout-[^-]+-[^-]+-(.+)\.jsonl$')
    if ($match.Success) { return $match.Groups[1].Value }
    return [IO.Path]::GetFileNameWithoutExtension($Path)
}

function Read-JsonLineFile {
    param([string]$Path)
    if (!(Test-Path -LiteralPath $Path)) { return @() }
    $items = New-Object System.Collections.Generic.List[object]
    foreach ($line in [IO.File]::ReadLines($Path)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try { $items.Add(($line | ConvertFrom-Json -Depth 100)) } catch { }
    }
    return $items
}

function Get-ThreadNames {
    $path = Join-Path $Script:CodexHome 'session_index.jsonl'
    $names = @{}
    foreach ($obj in Read-JsonLineFile -Path $path) {
        if ($obj.PSObject.Properties['id'] -and $obj.PSObject.Properties['thread_name']) {
            $names[[string]$obj.id] = [string]$obj.thread_name
        }
    }
    return $names
}

function Get-SessionPaths {
    param([bool]$IncludeArchived)
    $paths = New-Object System.Collections.Generic.List[string]
    $sessionsDir = Join-Path $Script:CodexHome 'sessions'
    if (Test-Path -LiteralPath $sessionsDir) {
        Get-ChildItem -LiteralPath $sessionsDir -Recurse -File -Filter 'rollout-*.jsonl' -ErrorAction SilentlyContinue | ForEach-Object { $paths.Add($_.FullName) }
    }
    if ($IncludeArchived) {
        $archivedDir = Join-Path $Script:CodexHome 'archived_sessions'
        if (Test-Path -LiteralPath $archivedDir) {
            Get-ChildItem -LiteralPath $archivedDir -File -Filter 'rollout-*.jsonl' -ErrorAction SilentlyContinue | ForEach-Object { $paths.Add($_.FullName) }
        }
        if (Test-Path -LiteralPath $Script:CodexHome) {
            Get-ChildItem -LiteralPath $Script:CodexHome -Directory -Filter 'restore-backup-*' -ErrorAction SilentlyContinue | ForEach-Object {
                Get-ChildItem -LiteralPath $_.FullName -File -Filter 'rollout-*.jsonl' -ErrorAction SilentlyContinue | ForEach-Object { $paths.Add($_.FullName) }
            }
        }
    }
    return @($paths | Sort-Object)
}

function Parse-Session {
    param([string]$Path)
    $messages = New-Object System.Collections.Generic.List[object]
    $errors = New-Object System.Collections.Generic.List[string]
    $eventTypes = New-Object System.Collections.Generic.List[string]
    $session = [ordered]@{
        Id = ''
        Path = $Path
        FirstTimestamp = ''
        LastTimestamp = ''
        Cwd = ''
        Provider = ''
        Model = ''
        Source = ''
        ThreadName = ''
        Messages = $messages
        Errors = $errors
        EventTypes = $eventTypes
    }

    foreach ($obj in Read-JsonLineFile -Path $Path) {
        $timestamp = if ($obj.PSObject.Properties['timestamp']) { [string]$obj.timestamp } else { '' }
        if ($timestamp) {
            if (!$session.FirstTimestamp) { $session.FirstTimestamp = $timestamp }
            $session.LastTimestamp = $timestamp
        }
        $recordType = if ($obj.PSObject.Properties['type']) { [string]$obj.type } else { '' }
        $payload = if ($obj.PSObject.Properties['payload']) { $obj.payload } else { $null }

        if ($recordType -eq 'session_meta' -and $null -ne $payload) {
            if ($payload.PSObject.Properties['id']) { $session.Id = [string]$payload.id }
            if ($payload.PSObject.Properties['cwd']) { $session.Cwd = [string]$payload.cwd }
            if ($payload.PSObject.Properties['model_provider']) { $session.Provider = [string]$payload.model_provider }
            if ($payload.PSObject.Properties['model']) { $session.Model = [string]$payload.model }
            if ($payload.PSObject.Properties['source']) { $session.Source = [string]$payload.source }
            elseif ($payload.PSObject.Properties['originator']) { $session.Source = [string]$payload.originator }
            continue
        }

        if ($recordType -eq 'turn_context' -and $null -ne $payload) {
            if ($payload.PSObject.Properties['cwd']) { $session.Cwd = [string]$payload.cwd }
            if ($payload.PSObject.Properties['model']) { $session.Model = [string]$payload.model }
            continue
        }

        if ($recordType -eq 'event_msg' -and $null -ne $payload) {
            $type = if ($payload.PSObject.Properties['type']) { [string]$payload.type } else { '' }
            if ($type) { $eventTypes.Add($type) }
            if ($type -eq 'error' -and $payload.PSObject.Properties['message']) {
                $errors.Add((Protect-SecretText ([string]$payload.message)))
            }
            if ($type -eq 'user_message' -and $payload.PSObject.Properties['message']) {
                $text = Protect-SecretText ([string]$payload.message)
                if ($text -and !(Test-NoiseMessage $text)) {
                    $messages.Add([pscustomobject]@{ Timestamp = $timestamp; Role = 'user'; Text = $text; Source = 'event_msg' })
                }
            }
            continue
        }

        if ($recordType -eq 'response_item' -and $null -ne $payload) {
            if (!($payload.PSObject.Properties['type']) -or [string]$payload.type -ne 'message') { continue }
            if (!($payload.PSObject.Properties['role'])) { continue }
            $role = [string]$payload.role
            if (@('user','assistant','tool','function') -notcontains $role) { continue }
            $content = if ($payload.PSObject.Properties['content']) { $payload.content } else { $null }
            $text = Protect-SecretText (Convert-ContentToText $content)
            if (!$text -or (Test-NoiseMessage $text)) { continue }
            if ($role -eq 'user') {
                $duplicate = $false
                $recent = @($messages | Select-Object -Last 3)
                foreach ($prev in $recent) {
                    if ($prev.Role -eq 'user' -and $prev.Text -eq $text) { $duplicate = $true; break }
                }
                if ($duplicate) { continue }
            }
            $messages.Add([pscustomobject]@{ Timestamp = $timestamp; Role = $role; Text = $text; Source = 'response_item' })
        }
    }

    if (!$session.Id) { $session.Id = Get-SessionIdFromFilename $Path }
    return [pscustomobject]$session
}

function Load-Sessions {
    param([bool]$IncludeArchived)
    $names = Get-ThreadNames
    $seenCurrent = @{}
    $loaded = New-Object System.Collections.Generic.List[object]
    foreach ($path in Get-SessionPaths -IncludeArchived $IncludeArchived) {
        $s = Parse-Session -Path $path
        if (!$s.Id) { continue }
        if ($names.ContainsKey($s.Id)) { $s.ThreadName = $names[$s.Id] }
        $isArchive = $path -match '\\archived_sessions\\|\\restore-backup-'
        if ($isArchive -and $seenCurrent.ContainsKey($s.Id)) { continue }
        if (!$isArchive) { $seenCurrent[$s.Id] = $true }
        $loaded.Add($s)
    }
    return @($loaded | Sort-Object @{ Expression = { if ($_.FirstTimestamp) { $_.FirstTimestamp } else { [IO.Path]::GetFileName($_.Path) } } })
}

function Get-UserTurns { param($Session) return @($Session.Messages | Where-Object Role -eq 'user').Count }
function Get-AssistantTurns { param($Session) return @($Session.Messages | Where-Object Role -eq 'assistant').Count }
function Get-AbortedTurns { param($Session) return @($Session.EventTypes | Where-Object { $_ -eq 'turn_aborted' }).Count }
function Get-RolledBackTurns { param($Session) return @($Session.EventTypes | Where-Object { $_ -eq 'thread_rolled_back' }).Count }

function Read-TomlValue {
    param([string]$Line)
    $value = ($Line -split '=', 2)[1].Trim()
    if ($value.StartsWith('"') -and $value.EndsWith('"') -and $value.Length -ge 2) { return $value.Substring(1, $value.Length - 2) }
    return $value
}

function Read-ConfigSummary {
    $path = Join-Path $Script:CodexHome 'config.toml'
    $values = @{}
    $providers = New-Object System.Collections.Generic.HashSet[string]
    if (!(Test-Path -LiteralPath $path)) { return [pscustomobject]@{ Path = $path; Values = $values; Providers = $providers } }
    $current = ''
    $inCurrent = $false
    foreach ($raw in Get-Content -LiteralPath $path -ErrorAction SilentlyContinue) {
        $line = $raw.Trim()
        if (!$line -or $line.StartsWith('#')) { continue }
        if ($line -match '^model_provider\s*=') { $values['model_provider'] = Read-TomlValue $line; $current = $values['model_provider']; continue }
        if ($line -match '^disable_response_storage\s*=') { $values['disable_response_storage'] = Read-TomlValue $line; continue }
        if ($line -match '^\[model_providers\.([^\]]+)\]') {
            $name = $Matches[1].Trim('"')
            [void]$providers.Add($name)
            $inCurrent = ($name -eq $current)
            continue
        }
        if ($inCurrent -and $line -match '^base_url\s*=') { $values['base_url'] = Read-TomlValue $line }
        if ($inCurrent -and $line -match '^wire_api\s*=') { $values['wire_api'] = Read-TomlValue $line }
    }
    return [pscustomobject]@{ Path = $path; Values = $values; Providers = $providers }
}

function Read-AgentProvider {
    param([string]$Path)
    if (!(Test-Path -LiteralPath $Path)) { return '' }
    foreach ($raw in Get-Content -LiteralPath $Path -ErrorAction SilentlyContinue) {
        $line = $raw.Trim()
        if ($line -match '^model_provider\s*=') { return Read-TomlValue $line }
    }
    return ''
}

function Quote-TomlString { param([string]$Value) return '"' + ($Value -replace '\\','\\' -replace '"','\"') + '"' }
function Quote-ShellArg { param([string]$Value) if ($Value -match '^[A-Za-z0-9_@%+=:,./\\-]+$') { return $Value } return '"' + ($Value -replace '"','\"') + '"' }

function Render-SessionDetail {
    param($Session, [int]$Prompts = 20)
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("id:        $($Session.Id)")
    $lines.Add("name:      $(if ($Session.ThreadName) { $Session.ThreadName } else { 'unnamed' })")
    $lines.Add("path:      $($Session.Path)")
    $lines.Add("time:      $($Session.FirstTimestamp) -> $($Session.LastTimestamp)")
    $lines.Add("cwd:       $(if ($Session.Cwd) { $Session.Cwd } else { 'unknown' })")
    $lines.Add("provider:  $(if ($Session.Provider) { $Session.Provider } else { 'unknown' })")
    $lines.Add("model:     $(if ($Session.Model) { $Session.Model } else { 'unknown' })")
    $lines.Add("source:    $(if ($Session.Source) { $Session.Source } else { 'unknown' })")
    $lines.Add("turns:     user=$(Get-UserTurns $Session) assistant=$(Get-AssistantTurns $Session)")
    $lines.Add("events:    aborted=$(Get-AbortedTurns $Session) rolled_back=$(Get-RolledBackTurns $Session)")
    if ($Session.Errors.Count -gt 0) {
        $lines.Add('errors:')
        foreach ($e in @($Session.Errors | Select-Object -First 5)) { $lines.Add('  - ' + (ConvertTo-OneLine $e 180)) }
    }
    $users = @($Session.Messages | Where-Object Role -eq 'user')
    if ($users.Count -gt 0) {
        $lines.Add('recent user prompts:')
        foreach ($m in @($users | Select-Object -Last $Prompts)) { $lines.Add('  - ' + (ConvertTo-CompactTime $m.Timestamp) + ' ' + (ConvertTo-OneLine $m.Text 220)) }
    }
    return ($lines -join "`n") + "`n"
}

function Render-RestorePrompt {
    param($Session)
    $latestUser = @($Session.Messages | Where-Object Role -eq 'user' | Select-Object -Last 1)
    $title = if ($Session.ThreadName) { $Session.ThreadName } else { $Session.Id }
    $lines = New-Object System.Collections.Generic.List[string]
    @(
        '# Codex Local Session Restore','',
        'Please restore the local Codex session below and continue the work.','',
        'Do not ask me to paste the full transcript manually. Read the local JSONL session file if needed and extract the latest goal, constraints, completed work, remaining tasks, and errors.','',
        '## Session','',
        "- Session id: ``$($Session.Id)``",
        "- Thread name: ``$title``",
        "- Original cwd: ``$(if ($Session.Cwd) { $Session.Cwd } else { 'unknown' })``",
        "- Original provider: ``$(if ($Session.Provider) { $Session.Provider } else { 'unknown' })``",
        "- Time range: ``$($Session.FirstTimestamp)`` -> ``$($Session.LastTimestamp)``",
        "- Local JSONL: ``$($Session.Path)``",'',
        '## Restore Requirements','',
        '1. Read `Local JSONL` first; do not rely on server-side `/resume`.',
        '2. Summarize the latest target, key constraints, modified/relevant files, verification status, and remaining work.',
        '3. Treat the last real user request as the current task.',
        '4. If a workspace path is mentioned, check whether the current workspace matches before editing.'
    ) | ForEach-Object { $lines.Add($_) }
    if ($latestUser.Count -gt 0) { @('', '## Last User Request', '', $latestUser[0].Text) | ForEach-Object { $lines.Add($_) } }
    if ($Session.Errors.Count -gt 0) {
        @('', '## Recorded Errors', '') | ForEach-Object { $lines.Add($_) }
        foreach ($e in @($Session.Errors | Select-Object -First 5)) { $lines.Add('- ' + (ConvertTo-OneLine $e 220)) }
    }
    return (($lines -join "`n").TrimEnd() + "`n")
}

function Render-NativeResume {
    param($Session)
    $config = Read-ConfigSummary
    $current = if ($config.Values.ContainsKey('model_provider')) { $config.Values['model_provider'] } else { '' }
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("session:          $($Session.Id) ($(if ($Session.ThreadName) { $Session.ThreadName } else { 'unnamed' }))")
    $lines.Add("session provider: $(if ($Session.Provider) { $Session.Provider } else { 'unknown' })")
    $lines.Add("current provider: $(if ($current) { $current } else { 'unknown' })")
    $lines.Add("config:           $($config.Path)")
    if (!$Session.Provider) {
        $lines.Add('native status:    blocked')
        $lines.Add('reason:           session JSONL does not record its original provider')
        $lines.Add('fallback:         use Export Restore File or Copy Restore Prompt')
        return ($lines -join "`n") + "`n"
    }
    if (!$config.Providers.Contains($Session.Provider)) {
        $lines.Add('native status:    blocked')
        $lines.Add("reason:           provider '$($Session.Provider)' is not defined in current config")
        $lines.Add('what to do:       restore/add the old provider block first, then run the command below')
        $lines.Add('candidate command after provider is restored:')
    } else {
        $lines.Add('native status:    possible')
        $lines.Add('note:             this still requires provider-side response-chain resume support')
    }
    $args = @('codex','resume',$Session.Id,'-c',('model_provider=' + (Quote-TomlString $Session.Provider)),'-c','disable_response_storage=false')
    if ($Session.Cwd) { $args += @('-C', $Session.Cwd) }
    $lines.Add('command:')
    $lines.Add('  ' + (($args | ForEach-Object { Quote-ShellArg $_ }) -join ' '))
    $lines.Add('fallback:')
    $lines.Add('  use Export Restore File or Copy Restore Prompt')
    return ($lines -join "`n") + "`n"
}

function Render-Doctor {
    param($Session)
    $config = Read-ConfigSummary
    $current = if ($config.Values.ContainsKey('model_provider')) { $config.Values['model_provider'] } else { '' }
    $risks = New-Object System.Collections.Generic.List[string]
    if ($current -and $Session.Provider -and $current -ne $Session.Provider) { $risks.Add("provider changed: session used '$($Session.Provider)', current config uses '$current'") }
    if ($config.Values.ContainsKey('disable_response_storage') -and $config.Values['disable_response_storage'] -eq 'true') { $risks.Add('disable_response_storage is true, so server-side response-chain resume may be unavailable') }
    if ($config.Values.ContainsKey('wire_api') -and $config.Values['wire_api'] -eq 'responses') { $risks.Add('current provider uses the Responses API; provider compatibility must include response storage/readback') }
    if ((Get-AssistantTurns $Session) -eq 0) { $risks.Add('session has no completed assistant messages in local JSONL; it may have been interrupted before a resumable response existed') }
    if ((Get-AbortedTurns $Session) -gt 0) { $risks.Add("session records $(Get-AbortedTurns $Session) aborted turn(s)") }
    if ((Get-RolledBackTurns $Session) -gt 0) { $risks.Add("session records $(Get-RolledBackTurns $Session) rollback event(s)") }
    foreach ($e in $Session.Errors) {
        if ($e -match '/v1/responses|Invalid URL|Bad Gateway') { $risks.Add('session already recorded API error: ' + (ConvertTo-OneLine $e 160)); break }
        if ($e -match 'stream disconnected before completion') { $risks.Add('session recorded a stream that closed before response.completed'); break }
    }
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("session:          $($Session.Id) ($(if ($Session.ThreadName) { $Session.ThreadName } else { 'unnamed' }))")
    $lines.Add("session provider: $(if ($Session.Provider) { $Session.Provider } else { 'unknown' })")
    $lines.Add("current provider: $(if ($current) { $current } else { 'unknown' })")
    $baseUrl = if ($config.Values.ContainsKey('base_url')) { $config.Values['base_url'] } else { 'unknown' }
    $wireApi = if ($config.Values.ContainsKey('wire_api')) { $config.Values['wire_api'] } else { 'unknown' }
    $storageDisabled = if ($config.Values.ContainsKey('disable_response_storage')) { $config.Values['disable_response_storage'] } else { 'unknown' }
    $lines.Add("current base_url: $baseUrl")
    $lines.Add("wire_api:         $wireApi")
    $lines.Add("storage disabled: $storageDisabled")
    $lines.Add("provider exists:  $(if ($Session.Provider -and $config.Providers.Contains($Session.Provider)) { 'true' } else { 'false' })")
    $lines.Add("assistant turns:  $(Get-AssistantTurns $Session)")
    $lines.Add("aborted turns:    $(Get-AbortedTurns $Session)")
    $lines.Add("rollback events:  $(Get-RolledBackTurns $Session)")
    if ($risks.Count -eq 0) { $lines.Add('resume risk:      no obvious local risk found') }
    else {
        $lines.Add('resume risk:      high')
        $lines.Add('why:')
        foreach ($r in @($risks | Select-Object -Unique)) { $lines.Add('  - ' + $r) }
        $lines.Add('native-first:')
        if ($Session.Provider -and $config.Providers.Contains($Session.Provider)) { $lines.Add("  codex resume $($Session.Id) -c model_provider=$(Quote-TomlString $Session.Provider) -c disable_response_storage=false") }
        elseif ($Session.Provider) { $lines.Add("  missing provider definition for '$($Session.Provider)' in $($config.Path)") }
        else { $lines.Add('  session does not record an original provider') }
        $lines.Add('fallback:')
        $lines.Add('  use Export Restore File or Copy Restore Prompt')
    }
    return ($lines -join "`n") + "`n"
}

function Get-ProviderSyncReport {
    param([bool]$DryRun, [bool]$IncludeSessions = $true, [bool]$IncludeArchived = $false)
    $config = Read-ConfigSummary
    $current = if ($config.Values.ContainsKey('model_provider')) { $config.Values['model_provider'] } else { '' }
    if (!$current -or !$config.Providers.Contains($current)) { throw 'current model_provider is missing or not defined' }
    $timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $backupRoot = Join-Path $Script:CodexHome "provider-sync-backup-$timestamp"
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("current provider: $current")
    $lines.Add('defined providers: ' + (($config.Providers | Sort-Object) -join ', '))
    $lines.Add('dry run:          ' + ($(if ($DryRun) { 'true' } else { 'false' })))

    $agentChanges = 0
    $agentsDir = Join-Path $Script:CodexHome 'agents'
    if (Test-Path -LiteralPath $agentsDir) {
        foreach ($file in Get-ChildItem -LiteralPath $agentsDir -File -Filter '*.toml' -ErrorAction SilentlyContinue | Sort-Object FullName) {
            $provider = Read-AgentProvider $file.FullName
            if ($provider -and $provider -ne $current) {
                $agentChanges++
                $lines.Add("  agent $($file.Name): $provider -> $current")
                if (!$DryRun) {
                    $backupDir = Join-Path $backupRoot 'agents'
                    New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
                    Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $backupDir $file.Name) -Force
                    $text = Get-Content -LiteralPath $file.FullName -Raw
                    $updated = [regex]::Replace($text, '(?m)^model_provider\s*=\s*"[^"]+"', 'model_provider = ' + (Quote-TomlString $current), 1)
                    Set-Content -LiteralPath $file.FullName -Value $updated -NoNewline -Encoding UTF8
                }
            }
        }
    }
    $lines.Add("agent refs:       $agentChanges change(s)")

    $sessionChanges = 0
    if ($IncludeSessions) {
        foreach ($path in Get-SessionPaths -IncludeArchived $IncludeArchived) {
            $raw = if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path -Raw } else { '' }
            $outLines = New-Object System.Collections.Generic.List[string]
            $changed = $false
            $oldProvider = ''
            foreach ($line in ($raw -split "`r?`n")) {
                if (!$line) { $outLines.Add($line); continue }
                try { $obj = $line | ConvertFrom-Json -Depth 100 } catch { $outLines.Add($line); continue }
                if ($obj.PSObject.Properties['type'] -and $obj.type -eq 'session_meta' -and $obj.PSObject.Properties['payload']) {
                    $payload = $obj.payload
                    if ($payload.PSObject.Properties['model_provider']) {
                        $oldProvider = [string]$payload.model_provider
                        if ($oldProvider -and $oldProvider -ne $current) {
                            $payload.model_provider = $current
                            $outLines.Add(($obj | ConvertTo-Json -Depth 100 -Compress))
                            $changed = $true
                            continue
                        }
                    }
                }
                $outLines.Add($line)
            }
            if ($changed) {
                $sessionChanges++
                $lines.Add("  session $([IO.Path]::GetFileName($path)): $oldProvider -> $current")
                if (!$DryRun) {
                    $backupDir = Join-Path $backupRoot 'sessions'
                    New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
                    Copy-Item -LiteralPath $path -Destination (Join-Path $backupDir ([IO.Path]::GetFileName($path))) -Force
                    Set-Content -LiteralPath $path -Value (($outLines -join "`n") + "`n") -NoNewline -Encoding UTF8
                }
            }
        }
        $lines.Add("session refs:     $sessionChanges change(s)")
    } else {
        $lines.Add('session refs:     skipped')
    }
    if (!$DryRun -and ($agentChanges -gt 0 -or $sessionChanges -gt 0)) { $lines.Add("backup:           $backupRoot") }
    elseif ($agentChanges -eq 0 -and $sessionChanges -eq 0) { $lines.Add('status:           already synced') }
    return ($lines -join "`n") + "`n"
}

function Update-List {
    try {
        $window.Cursor = [System.Windows.Input.Cursors]::Wait
        $status.Text = 'Loading sessions...'
        $Script:Sessions = @(Load-Sessions -IncludeArchived ([bool]$includeArchived.IsChecked))
        $list.Items.Clear()
        foreach ($s in $Script:Sessions) {
            $name = if ($s.ThreadName) { $s.ThreadName } else { 'unnamed' }
            $cwd = if ($s.Cwd) { $s.Cwd } else { 'unknown cwd' }
            $item = New-Object System.Windows.Controls.ListBoxItem
            $item.Tag = $s
            $item.Content = "$(ConvertTo-CompactTime $s.LastTimestamp)  $($s.Provider)  turns $(Get-UserTurns $s)`n$name`n$($s.Id)`n$cwd"
            $list.Items.Add($item) | Out-Null
        }
        if ($list.Items.Count -gt 0) { $list.SelectedIndex = 0 } else { $details.Text = 'No local sessions found.' }
        $status.Text = "Loaded $($Script:Sessions.Count) session(s)."
    } catch {
        $details.Text = 'error: ' + $_.Exception.Message
        $status.Text = 'Load failed.'
    } finally {
        $window.Cursor = $null
    }
}

function Set-Preview {
    param($Session)
    $Script:SelectedSession = $Session
    if ($null -eq $Session) { return }
    $details.Text = @"
Selected session

id:       $($Session.Id)
updated:  $(ConvertTo-CompactTime $Session.LastTimestamp)
provider: $($Session.Provider)
turns:    $(Get-UserTurns $Session)
context:  $(if ($Session.ThreadName) { $Session.ThreadName } else { 'unnamed' }) | $(if ($Session.Cwd) { $Session.Cwd } else { 'unknown cwd' })

Click Show Detail, Diagnose /resume Risk, Native Resume Command, or Provider Sync actions.
"@
}

[xml]$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="Codex Continuity for Windows" Height="760" Width="1160" MinHeight="620" MinWidth="980" WindowStartupLocation="CenterScreen">
  <DockPanel>
    <Border DockPanel.Dock="Top" Padding="12" Background="#F7F9FC" BorderBrush="#D8DEE9" BorderThickness="0,0,0,1">
      <DockPanel LastChildFill="False">
        <StackPanel Orientation="Vertical" DockPanel.Dock="Left">
          <TextBlock Text="Codex Continuity" FontSize="22" FontWeight="SemiBold" Foreground="#1F2937" />
          <TextBlock Text="Keep local Codex sessions resumable after provider switches" Foreground="#667085" />
        </StackPanel>
        <StackPanel Orientation="Horizontal" DockPanel.Dock="Right" VerticalAlignment="Center">
          <CheckBox x:Name="IncludeArchived" Content="Include archived" Margin="0,0,12,0" VerticalAlignment="Center" />
          <Button x:Name="RefreshButton" Content="Refresh" Width="92" Height="32" Margin="0,0,8,0" />
          <Button x:Name="OpenSessionsButton" Content="Open sessions" Width="120" Height="32" />
        </StackPanel>
      </DockPanel>
    </Border>
    <StatusBar DockPanel.Dock="Bottom"><TextBlock x:Name="StatusText" Text="Ready" /></StatusBar>
    <Grid>
      <Grid.ColumnDefinitions>
        <ColumnDefinition Width="390" />
        <ColumnDefinition Width="*" />
      </Grid.ColumnDefinitions>
      <Border Grid.Column="0" BorderBrush="#D8DEE9" BorderThickness="0,0,1,0">
        <DockPanel>
          <TextBox x:Name="SearchBox" DockPanel.Dock="Top" Margin="10" Height="30" VerticalContentAlignment="Center" ToolTip="Search session id, name, provider, or cwd" />
          <ListBox x:Name="SessionList" Margin="10,0,10,10" FontFamily="Consolas" FontSize="12" />
        </DockPanel>
      </Border>
      <DockPanel Grid.Column="1">
        <StackPanel DockPanel.Dock="Top" Margin="10">
          <WrapPanel Margin="0,0,0,8">
            <Button x:Name="NativeButton" Content="Native Resume Command" Height="34" Margin="0,0,8,8" Padding="12,0" />
            <Button x:Name="DoctorButton" Content="Diagnose /resume Risk" Height="34" Margin="0,0,8,8" Padding="12,0" />
            <Button x:Name="DetailButton" Content="Show Detail" Height="34" Margin="0,0,8,8" Padding="12,0" />
            <Button x:Name="PreviewSyncButton" Content="Preview Sync" Height="34" Margin="0,0,8,8" Padding="12,0" />
            <Button x:Name="SyncButton" Content="Sync to Current Provider" Height="34" Margin="0,0,8,8" Padding="12,0" />
            <Button x:Name="ExportButton" Content="Export Restore File" Height="34" Margin="0,0,8,8" Padding="12,0" />
            <Button x:Name="CopyButton" Content="Copy Restore Prompt" Height="34" Margin="0,0,8,8" Padding="12,0" />
          </WrapPanel>
        </StackPanel>
        <TextBox x:Name="DetailsBox" Margin="10,0,10,10" FontFamily="Consolas" FontSize="13" AcceptsReturn="True" AcceptsTab="True" TextWrapping="Wrap" VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Auto" IsReadOnly="True" />
      </DockPanel>
    </Grid>
  </DockPanel>
</Window>
'@

$reader = New-Object System.Xml.XmlNodeReader $xaml
$window = [Windows.Markup.XamlReader]::Load($reader)
$list = $window.FindName('SessionList')
$details = $window.FindName('DetailsBox')
$status = $window.FindName('StatusText')
$includeArchived = $window.FindName('IncludeArchived')
$search = $window.FindName('SearchBox')

$window.FindName('RefreshButton').Add_Click({ Update-List })
$includeArchived.Add_Click({ Update-List })
$window.FindName('OpenSessionsButton').Add_Click({
    $path = Join-Path $Script:CodexHome 'sessions'
    if (!(Test-Path -LiteralPath $path)) { New-Item -ItemType Directory -Force -Path $path | Out-Null }
    Start-Process explorer.exe $path
})
$list.Add_SelectionChanged({ if ($list.SelectedItem) { Set-Preview $list.SelectedItem.Tag } })
$search.Add_TextChanged({
    $needle = $search.Text.ToLowerInvariant()
    $list.Items.Clear()
    foreach ($s in $Script:Sessions) {
        $hay = (($s.Id + ' ' + $s.ThreadName + ' ' + $s.Provider + ' ' + $s.Cwd).ToLowerInvariant())
        if (!$needle -or $hay.Contains($needle)) {
            $name = if ($s.ThreadName) { $s.ThreadName } else { 'unnamed' }
            $cwd = if ($s.Cwd) { $s.Cwd } else { 'unknown cwd' }
            $item = New-Object System.Windows.Controls.ListBoxItem
            $item.Tag = $s
            $item.Content = "$(ConvertTo-CompactTime $s.LastTimestamp)  $($s.Provider)  turns $(Get-UserTurns $s)`n$name`n$($s.Id)`n$cwd"
            $list.Items.Add($item) | Out-Null
        }
    }
})

$window.FindName('DetailButton').Add_Click({ if ($Script:SelectedSession) { $details.Text = Render-SessionDetail $Script:SelectedSession 20 } })
$window.FindName('NativeButton').Add_Click({ if ($Script:SelectedSession) { $details.Text = Render-NativeResume $Script:SelectedSession } })
$window.FindName('DoctorButton').Add_Click({ if ($Script:SelectedSession) { $details.Text = Render-Doctor $Script:SelectedSession } })
$window.FindName('PreviewSyncButton').Add_Click({ try { $details.Text = Get-ProviderSyncReport -DryRun $true -IncludeSessions $true -IncludeArchived $false } catch { $details.Text = 'error: ' + $_.Exception.Message } })
$window.FindName('SyncButton').Add_Click({
    $answer = [System.Windows.MessageBox]::Show('This will backup and update Codex provider metadata under your .codex folder. Continue?', 'Sync Provider', 'OKCancel', 'Warning')
    if ($answer -eq 'OK') {
        try { $details.Text = Get-ProviderSyncReport -DryRun $false -IncludeSessions $true -IncludeArchived $false; Update-List } catch { $details.Text = 'error: ' + $_.Exception.Message }
    }
})
$window.FindName('ExportButton').Add_Click({
    if (!$Script:SelectedSession) { return }
    $downloads = Join-Path $env:USERPROFILE 'Downloads'
    if (!(Test-Path -LiteralPath $downloads)) { $downloads = $env:USERPROFILE }
    $out = Join-Path $downloads ("codex-restore-" + $Script:SelectedSession.Id.Substring(0, [Math]::Min(8, $Script:SelectedSession.Id.Length)) + ".md")
    try {
        Render-RestorePrompt $Script:SelectedSession | Set-Content -LiteralPath $out -Encoding UTF8
        $details.Text = "Wrote restoration prompt: $out"
        Start-Process explorer.exe "/select,`"$out`""
    } catch { $details.Text = 'error: ' + $_.Exception.Message }
})
$window.FindName('CopyButton').Add_Click({
    if (!$Script:SelectedSession) { return }
    $prompt = Render-RestorePrompt $Script:SelectedSession
    [System.Windows.Clipboard]::SetText($prompt)
    $details.Text = "Restore prompt copied to clipboard.`n`n$prompt"
})

Update-List
[void]$window.ShowDialog()
