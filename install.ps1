#Requires -Version 5.1
<#
.SYNOPSIS
    HackCode installer for Windows 10 / 11 PowerShell.

.DESCRIPTION
    Installs HackCode entirely per-user, no admin rights required, no Cygwin,
    no WSL, no Git Bash. Mirrors the behaviour of `install.sh` on Linux/macOS:

        * downloads the prebuilt `hackcode-windows-x64.zip` from GitHub Releases
          when available; otherwise builds from source via `cargo build --release`
        * installs the binary to %LOCALAPPDATA%\Programs\HackCode\hackcode.exe
        * appends that directory to the **user** PATH (no machine PATH writes)
        * installs Ollama if `winget` or `scoop` are available, otherwise
          prints a one-liner the user can run themselves
        * pulls the recommended Ollama model based on installed RAM
        * creates the `hackcode-uncensored` alias used by the CLI

    All install destinations are user-writable, so this works inside isolated
    environments such as Windows Sandbox, restricted corporate machines, and
    locked-down VMs.

.PARAMETER Source
    Build from source instead of trying GitHub Releases. Useful when running
    the installer from a local checkout of the repo.

.PARAMETER SkipModel
    Skip the Ollama install + model pull step. Useful when you already have
    Ollama configured the way you want it.

.PARAMETER OfflineSource
    Path to a pre-downloaded copy of the HackCode source tree (the directory
    containing the `rust/` folder). When set, the installer skips every
    network call to GitHub and builds directly from the local copy. Use this
    on machines where outbound HTTPS is blocked or filtered (e.g. corporate
    proxies / Zscaler that block raw.githubusercontent.com). Workflow:

        1. On any internet-connected machine:
           git clone --branch dev https://github.com/johnesecat/hackcode-main.git C:\Temp\hackcode
           (or download the repo as a zip from GitHub and extract it)
        2. Copy the folder to the target machine (USB stick, OneDrive, etc.)
        3. On the target machine:
           .\install.ps1 -OfflineSource C:\Temp\hackcode

.PARAMETER OfflineZip
    Path to a pre-downloaded `hackcode-windows-*.zip` release artifact (or a
    zip that contains `hackcode.exe`). Skips the release-API call entirely.

.PARAMETER Proxy
    Outbound HTTPS proxy URL (e.g. `http://proxy.example.com:8080`). Falls
    back to `$env:HTTPS_PROXY` / `$env:HTTP_PROXY` when omitted. Passed to
    Invoke-WebRequest / Invoke-RestMethod for every network call.

.PARAMETER ProxyUseDefaultCredentials
    Authenticate to the proxy using the current Windows user's credentials.
    Required for most corporate NTLM / Kerberos proxies (incl. Zscaler ZIA
    when running in transparent-mode but proxy-authenticating).

.PARAMETER Repo
    Override the GitHub repository slug (default: `johnesecat/hackcode-main`).
    Useful when running the installer against a fork.

.EXAMPLE
    iwr https://raw.githubusercontent.com/johnesecat/hackcode-main/dev/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Source

.EXAMPLE
    # Offline install on a network-restricted machine:
    .\install.ps1 -OfflineSource C:\Temp\hackcode -SkipModel

.EXAMPLE
    # Through a corporate proxy with NTLM auth:
    .\install.ps1 -Proxy http://proxy.corp:8080 -ProxyUseDefaultCredentials

.NOTES
    Re-running the script is idempotent.
#>
[CmdletBinding()]
param(
    [switch]$Source,
    [switch]$SkipModel,
    [string]$OfflineSource,
    [string]$OfflineZip,
    [string]$Proxy,
    [switch]$ProxyUseDefaultCredentials,
    [string]$Repo = 'johnesecat/hackcode-main'
)

$ErrorActionPreference = 'Stop'

# Render UTF-8 box-drawing glyphs correctly on Windows PowerShell 5.1's
# default OEM code page console.
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}

# Resolve a repo checkout next to the running script. `$PSScriptRoot` is empty
# when the script is piped through `iex`, so guard every consumer with this
# helper rather than calling `Join-Path $PSScriptRoot ...` directly.
function Get-LocalCheckoutDir {
    if (-not [string]::IsNullOrEmpty($PSScriptRoot)) { return $PSScriptRoot }
    if ($PSCommandPath) {
        $parent = Split-Path -Parent $PSCommandPath
        if (-not [string]::IsNullOrEmpty($parent)) { return $parent }
    }
    if ($MyInvocation -and $MyInvocation.MyCommand -and $MyInvocation.MyCommand.Path) {
        $parent = Split-Path -Parent $MyInvocation.MyCommand.Path
        if (-not [string]::IsNullOrEmpty($parent)) { return $parent }
    }
    return $null
}

$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\HackCode'
$BinaryPath = Join-Path $InstallDir 'hackcode.exe'
$ConfigDir = Join-Path $env:APPDATA 'hackcode'
$SrcDir = Join-Path $env:USERPROFILE '.hackcode-src'

# Resolve proxy: explicit -Proxy beats env vars beats $null. HTTPS_PROXY is
# preferred over HTTP_PROXY since every URL we hit is HTTPS. Zscaler / other
# TLS-intercepting filters that need credential auth typically also require
# -ProxyUseDefaultCredentials to round-trip the current user's Windows token.
if (-not $Proxy) {
    foreach ($var in 'HTTPS_PROXY', 'https_proxy', 'HTTP_PROXY', 'http_proxy') {
        $candidate = [Environment]::GetEnvironmentVariable($var)
        if ($candidate) { $Proxy = $candidate; break }
    }
}

# Default web-request parameters that get splatted into every Invoke-WebRequest
# / Invoke-RestMethod call. Honors -Proxy / $env:HTTPS_PROXY and
# -ProxyUseDefaultCredentials. `-UseBasicParsing` keeps PS 5.1 from depending
# on Internet Explorer's COM engine (which is removed on some hardened SKUs).
function Get-WebRequestSplat {
    $splat = @{ UseBasicParsing = $true; ErrorAction = 'Stop' }
    if ($Proxy) { $splat['Proxy'] = $Proxy }
    if ($ProxyUseDefaultCredentials) { $splat['ProxyUseDefaultCredentials'] = $true }
    return $splat
}

# ─── Pretty printing ───────────────────────────────────────
# `e is only an ESC literal in PowerShell 6+. Build it explicitly so the
# colors render in stock Windows PowerShell 5.1 too (Win10/11 ConsoleHost
# supports virtual-terminal sequences when ANSI is emitted directly).
$ESC = [char]27
$Green = "$ESC[38;2;0;255;65m"
$Dim = "$ESC[90m"
$Bold = "$ESC[1m"
$Red = "$ESC[91m"
$Nc = "$ESC[0m"

function Write-Banner {
    Write-Host ''
    Write-Host "$Green ██╗  ██╗ █████╗  ██████╗██╗  ██╗ ██████╗ ██████╗ ██████╗ ███████╗$Nc"
    Write-Host "$Green ██║  ██║██╔══██╗██╔════╝██║ ██╔╝██╔════╝██╔═══██╗██╔══██╗██╔════╝$Nc"
    Write-Host "$Green ███████║███████║██║     █████╔╝ ██║     ██║   ██║██║  ██║█████╗  $Nc"
    Write-Host "$Green ██╔══██║██╔══██║██║     ██╔═██╗ ██║     ██║   ██║██║  ██║██╔══╝  $Nc"
    Write-Host "$Green ██║  ██║██║  ██║╚██████╗██║  ██╗╚██████╗╚██████╔╝██████╔╝███████╗$Nc"
    Write-Host "$Green ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝$Nc"
    Write-Host "$Green  >> AI-Powered Hacking Terminal  |  100% Local  |  No Censorship <<$Nc"
    Write-Host ''
}

function Step {
    param([int]$N, [int]$Total, [string]$Text)
    Write-Host "$Green[$N/$Total]$Nc $Text"
}

function Ok { param([string]$Text) Write-Host "  $Green$Text$Nc" }
function Info { param([string]$Text) Write-Host "  $Dim$Text$Nc" }
function Fail { param([string]$Text) Write-Host "  $Red$Text$Nc" }

# ─── Detect platform ───────────────────────────────────────
function Get-Architecture {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { 'x64' }
        'ARM64' { 'arm64' }
        default {
            throw "Unsupported architecture: $($env:PROCESSOR_ARCHITECTURE). HackCode supports x64 and arm64."
        }
    }
}

# ─── Add a directory to the user PATH (no admin needed) ────
function Add-ToUserPath {
    param([string]$Directory)

    $current = [Environment]::GetEnvironmentVariable('Path', [EnvironmentVariableTarget]::User)
    if ([string]::IsNullOrEmpty($current)) { $current = '' }

    $existing = $current -split ';' | Where-Object { $_ -ieq $Directory }
    if ($existing) {
        Info "Already in user PATH"
        return
    }

    $newValue = if ($current.TrimEnd(';')) {
        "$($current.TrimEnd(';'));$Directory"
    } else {
        $Directory
    }
    [Environment]::SetEnvironmentVariable('Path', $newValue, [EnvironmentVariableTarget]::User)
    # Make this session see the new entry too.
    if (-not ($env:Path -split ';' | Where-Object { $_ -ieq $Directory })) {
        $env:Path = "$Directory;$env:Path"
    }
    Ok "Added to user PATH"
}

function Get-RamGb {
    try {
        $bytes = (Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop).TotalPhysicalMemory
        return [int][math]::Floor($bytes / 1GB)
    } catch {
        return 0
    }
}

function Test-Command {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

# ─── Step 1: detect ────────────────────────────────────────
Write-Banner
$Arch = Get-Architecture
$Artifact = "hackcode-windows-$Arch"
Step 1 5 "Detected: ${Bold}Windows $Arch$Nc -> $Artifact"

# ─── Step 2: download or build ─────────────────────────────
Step 2 5 'Getting HackCode...'
$installed = $false

if ($Proxy) { Info "Using proxy: $Proxy" }

# 2a. Pre-downloaded zip wins over everything — no network calls at all.
if ($OfflineZip) {
    if (-not (Test-Path $OfflineZip)) {
        Fail "OfflineZip not found: $OfflineZip"
        throw "OfflineZip not found"
    }
    Info "Installing from local zip: $OfflineZip"
    $tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "hackcode-$([guid]::NewGuid())") -Force
    try {
        Expand-Archive -Path $OfflineZip -DestinationPath $tmp -Force
        $candidate = Get-ChildItem -Path $tmp -Filter 'hackcode.exe' -Recurse | Select-Object -First 1
        if (-not $candidate) {
            throw "OfflineZip $OfflineZip did not contain hackcode.exe"
        }
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Copy-Item -Path $candidate.FullName -Destination $BinaryPath -Force
        Ok "Installed from $OfflineZip"
        $installed = $true
    } finally {
        Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# 2b. Try GitHub Releases unless caller asked for source/offline-source build.
if (-not $installed -and -not $Source -and -not $OfflineSource) {
    try {
        $splat = Get-WebRequestSplat
        $tagInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
            -Headers @{ 'User-Agent' = 'hackcode-installer' } @splat
        $tag = $tagInfo.tag_name
        if ($tag) {
            $url = "https://github.com/$Repo/releases/download/$tag/$Artifact.zip"
            $tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "hackcode-$([guid]::NewGuid())") -Force
            try {
                $zipPath = Join-Path $tmp "$Artifact.zip"
                Invoke-WebRequest -Uri $url -OutFile $zipPath @splat
                Expand-Archive -Path $zipPath -DestinationPath $tmp -Force
                New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
                $candidate = Get-ChildItem -Path $tmp -Filter 'hackcode.exe' -Recurse | Select-Object -First 1
                if ($candidate) {
                    Copy-Item -Path $candidate.FullName -Destination $BinaryPath -Force
                    Ok "Downloaded $tag"
                    $installed = $true
                }
            } finally {
                Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    } catch {
        Info "No release artifact available ($($_.Exception.Message))."
        if ($_.Exception.Message -match 'Zscaler|blocked|policy|forbidden|filter') {
            Info 'Network appears to be filtered. See `Get-Help .\install.ps1 -Parameter OfflineSource` for the offline workflow.'
        }
    }
}

if (-not $installed) {
    Info 'Building from source...'

    if (-not (Test-Command 'cargo')) {
        Info 'Installing Rust toolchain (rustup-init.exe, user-scope, no admin)...'
        $rustInit = Join-Path $env:TEMP 'rustup-init.exe'
        try {
            $rustSplat = Get-WebRequestSplat
            Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $rustInit @rustSplat
        } catch {
            Fail "Could not download rustup-init.exe ($($_.Exception.Message))."
            Info 'If outbound HTTPS is filtered, install Rust manually from https://rustup.rs/ on an internet-connected machine, then re-run with -OfflineSource <path>.'
            throw
        }
        & $rustInit -y --default-toolchain stable --profile minimal | Out-Host
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    }

    # Resolve the source directory in priority order:
    #   1. Explicit -OfflineSource (offline / Zscaler workaround)
    #   2. Local checkout next to this script (resolved via Get-LocalCheckoutDir
    #      so we degrade gracefully when piped through `iex` and $PSScriptRoot
    #      is empty)
    #   3. git clone to $SrcDir
    $buildDir = $null
    if ($OfflineSource) {
        if (-not (Test-Path $OfflineSource)) {
            Fail "OfflineSource path does not exist: $OfflineSource"
            throw "OfflineSource not found"
        }
        $candidate = if (Test-Path (Join-Path $OfflineSource 'rust\Cargo.toml')) {
            Join-Path $OfflineSource 'rust'
        } elseif (Test-Path (Join-Path $OfflineSource 'Cargo.toml')) {
            $OfflineSource
        } else {
            $null
        }
        if (-not $candidate) {
            Fail "OfflineSource $OfflineSource does not look like a HackCode checkout (no rust\Cargo.toml or Cargo.toml found)."
            throw "OfflineSource does not contain a HackCode checkout"
        }
        Info "Using offline source: $candidate"
        $buildDir = $candidate
    }

    if (-not $buildDir) {
        $checkoutDir = Get-LocalCheckoutDir
        if ($checkoutDir) {
            $localManifest = Join-Path $checkoutDir 'rust\Cargo.toml'
            if (Test-Path $localManifest) {
                $buildDir = Join-Path $checkoutDir 'rust'
            }
        }
    }

    if (-not $buildDir) {
        if (-not (Test-Command 'git')) {
            Fail 'git is required to fetch HackCode source.'
            Info 'Install with `winget install Git.Git` or download from https://git-scm.com/download/win, then re-run this installer.'
            Info 'If outbound HTTPS is filtered, re-run with -OfflineSource <path> after downloading the repo zip on an unrestricted machine.'
            throw 'git not available'
        }
        if (Test-Path (Join-Path $SrcDir '.git')) {
            Info "Updating existing source checkout at $SrcDir"
            git -C $SrcDir fetch --quiet origin 2>$null | Out-Null
            $fetchExit = $LASTEXITCODE
            if ($fetchExit -ne 0) {
                Info "git fetch failed (exit code $fetchExit); building from existing checkout"
            }
            git -C $SrcDir reset --quiet --hard origin/HEAD 2>$null | Out-Null
            if ($LASTEXITCODE -ne 0) {
                throw "git reset failed with exit code $LASTEXITCODE"
            }
        } else {
            if (Test-Path $SrcDir) { Remove-Item $SrcDir -Recurse -Force }
            Info "Cloning https://github.com/$Repo.git into $SrcDir"
            git clone --quiet "https://github.com/$Repo.git" $SrcDir
            if ($LASTEXITCODE -ne 0) {
                Info 'Tip: if your network filters raw.githubusercontent.com but allows github.com, the clone usually still works.'
                Info 'Otherwise re-run with -OfflineSource <path-to-pre-downloaded-checkout>.'
                throw "git clone failed with exit code $LASTEXITCODE"
            }
        }
        $buildDir = Join-Path $SrcDir 'rust'
    }

    Info 'Compiling (~200 crates, this may take a few minutes)...'
    Push-Location $buildDir
    try {
        cargo build --release -p rusty-claude-cli
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $built = Join-Path $buildDir 'target\release\hackcode.exe'
    if (-not (Test-Path $built)) {
        Fail 'Build succeeded but hackcode.exe is missing.'
        throw "Expected $built"
    }
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path $built -Destination $BinaryPath -Force
    Ok 'Built and installed'
}

# ─── Step 3: PATH ──────────────────────────────────────────
Step 3 5 'Adding hackcode to PATH...'
Add-ToUserPath -Directory $InstallDir

# ─── Step 4: Ollama + model ────────────────────────────────
if ($SkipModel) {
    Step 4 5 'Skipping Ollama / model setup (--SkipModel)'
} else {
    Step 4 5 'Pulling AI model...'

    if (-not (Test-Command 'ollama')) {
        if (Test-Command 'winget') {
            Info 'Installing Ollama via winget (user scope, no admin)...'
            winget install --id Ollama.Ollama --accept-source-agreements --accept-package-agreements --silent --scope user
        } elseif (Test-Command 'scoop') {
            Info 'Installing Ollama via scoop...'
            scoop install ollama
        } else {
            Fail 'Ollama not found.'
            Info 'Install from https://ollama.ai/download/windows or run `winget install Ollama.Ollama` after installing winget.'
        }
    }

    if (Test-Command 'ollama') {
        Ok 'Ollama found'
        $existing = (& ollama list 2>$null) -join "`n"
        if ($existing -match 'hackcode-uncensored') {
            Ok 'hackcode-uncensored model ready'
        } else {
            $ramGb = Get-RamGb
            if ($ramGb -ge 24) {
                $baseModel = 'tripolskypetr/qwen3.5-uncensored-aggressive:35b'
                $desc = 'Qwen3.5-35B-A3B MoE Uncensored (~21GB)'
            } elseif ($ramGb -ge 8) {
                $baseModel = 'qwen3:8b'
                $desc = 'Qwen3-8B (~5GB)'
            } else {
                $baseModel = 'tripolskypetr/qwen3.5-uncensored-aggressive:4b'
                $desc = 'Qwen3.5-4B Uncensored (~3GB)'
            }
            Info "RAM: ${ramGb}GB - pulling ${desc}"

            $pulled = $false
            foreach ($candidate in @($baseModel, 'qwen3:8b', 'tripolskypetr/qwen3.5-uncensored-aggressive:4b', 'qwen3:4b')) {
                & ollama pull $candidate
                if ($LASTEXITCODE -eq 0) {
                    $baseModel = $candidate
                    $pulled = $true
                    break
                }
                Info "$candidate not available, trying next..."
            }

            if ($pulled) {
                Step 5 5 'Creating hackcode-uncensored model...'
                New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
                $modelfile = @"
FROM $baseModel
PARAMETER temperature 0.7
PARAMETER num_ctx 32768
"@
                $modelfilePath = Join-Path $ConfigDir 'Modelfile'
                Set-Content -Path $modelfilePath -Value $modelfile -Encoding ascii
                & ollama create hackcode-uncensored -f $modelfilePath
                Ok 'hackcode-uncensored ready'
            } else {
                Fail 'Could not pull any model.'
                Info 'Run: ollama pull qwen3:8b ; hackcode --setup'
            }
        }
    }
}

# ─── Done ──────────────────────────────────────────────────
Write-Host ''
Write-Host "$Green$('─' * 64)$Nc"
Write-Host "$Green[HackCode]$Nc Installation complete!"
Write-Host ''
Write-Host "  ${Bold}hackcode$Nc          ${Dim}# Start hacking$Nc"
Write-Host "  ${Bold}hackcode --help$Nc   ${Dim}# Show all commands$Nc"
Write-Host ''
Write-Host "  ${Dim}Open a new PowerShell window, or in this session run:$Nc"
Write-Host "  ${Bold}`$env:Path = '$InstallDir;' + `$env:Path$Nc"
Write-Host "$Green$('─' * 64)$Nc"
Write-Host ''
