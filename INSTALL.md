# Installation guide

## Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/johnesecat/hackcode-main/dev/install.sh | bash
```

The script installs everything per-user — no `sudo`, no admin, no global writes.

## Windows 10 / 11 (PowerShell, no admin)

```powershell
iwr https://raw.githubusercontent.com/johnesecat/hackcode-main/dev/install.ps1 | iex
```

This installs `hackcode.exe` to `%LOCALAPPDATA%\Programs\HackCode\`, adds it to
the **user** `PATH` (no machine PATH writes), drops the config under
`%APPDATA%\hackcode\`, and pulls an Ollama model sized for your machine. Stock
Windows PowerShell 5.1 is supported; PowerShell 7+ also works.

### Common flags

| Flag | What it does |
|---|---|
| `-Source` | Skip GitHub Releases, build from source via `cargo build --release`. |
| `-SkipModel` | Don't install Ollama or pull a model — leave that for `hackcode --setup`. |
| `-Repo <slug>` | Install from a fork (default: `johnesecat/hackcode-main`). |

## Network-restricted or filtered environments

Schools / corporate machines / Zscaler / locked-down VMs often block parts of
GitHub. The installer has three knobs to work around this. Pick whichever
matches your constraint.

### 1. Outbound proxy

If outbound HTTPS is allowed but only through a corporate proxy, point the
installer at it:

```powershell
.\install.ps1 -Proxy http://proxy.corp:8080 -ProxyUseDefaultCredentials
```

You can also set the standard `HTTPS_PROXY` environment variable and skip the
flags — the installer reads it automatically:

```powershell
$env:HTTPS_PROXY = 'http://proxy.corp:8080'
iwr https://raw.githubusercontent.com/johnesecat/hackcode-main/dev/install.ps1 | iex
```

`-ProxyUseDefaultCredentials` is required for most NTLM / Kerberos proxies
(passes your current Windows token through to the proxy).

### 2. Offline source build (recommended for Zscaler-style filtering)

When even GitHub is partially blocked (e.g. `raw.githubusercontent.com` is on
the deny-list but you have another machine with full internet):

**On the unrestricted machine:**

```powershell
# Either clone:
git clone --branch dev https://github.com/johnesecat/hackcode-main.git C:\Temp\hackcode
# …or download the repo zip:
Invoke-WebRequest `
  -Uri 'https://github.com/johnesecat/hackcode-main/archive/refs/heads/dev.zip' `
  -OutFile C:\Temp\hackcode.zip
Expand-Archive C:\Temp\hackcode.zip -DestinationPath C:\Temp\
```

Copy `C:\Temp\hackcode\` (or the extracted folder) plus `install.ps1` to a USB
stick / OneDrive / shared drive that the restricted machine can read.

**On the restricted machine:**

```powershell
.\install.ps1 -OfflineSource D:\hackcode -SkipModel
```

The installer will build entirely from the local copy and never reach out to
GitHub. The `-SkipModel` flag avoids the Ollama install + model pull (also
network-bound); run `hackcode --setup` later from somewhere with internet.

You still need Rust + cargo on the target machine. If `cargo` isn't already on
`PATH`, download `rustup-init.exe` from <https://rustup.rs> on the unrestricted
machine and copy it to the target before re-running.

### 3. Offline binary

If you have a pre-built `hackcode-windows-x64.zip` (released artifact downloaded
on another machine), point the installer at it directly:

```powershell
.\install.ps1 -OfflineZip D:\hackcode-windows-x64.zip
```

This skips the GitHub Releases lookup and the source build entirely.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `iwr : ... Hamilton-Wentworth ... Zscaler ... Website blocked` | Your network blocks `raw.githubusercontent.com`. Use the **Offline source build** workflow above, or set `$env:HTTPS_PROXY` if you have a proxy. |
| `iex : Cannot bind argument to parameter 'Path' because it is an empty string.` | Fixed in PR #2 — re-run the latest `install.ps1`. |
| Banner renders as `e[38;2;0;255;65m...` literally | Fixed in PR #2 — re-run the latest `install.ps1`. |
| Box-drawing chars render as `?` | Fixed in PR #2 — re-run the latest `install.ps1`. |
| `cargo build failed with exit code …` and no other clue | Re-run with `-Verbose` and check the build output. Most build failures on Windows are caused by a missing C linker; install `winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools"` then re-run. |
| Installer can't find `git` | `winget install Git.Git` (no admin needed) or download from <https://git-scm.com/download/win>. |

## What the installer writes

Everything lands in user-writable locations:

- Binary: `%LOCALAPPDATA%\Programs\HackCode\hackcode.exe`
- Config: `%APPDATA%\hackcode\config.json` (+ `Modelfile`)
- Source checkout (only if building from source via `git clone`):
  `%USERPROFILE%\.hackcode-src\`
- User `PATH` entry: `%LOCALAPPDATA%\Programs\HackCode`

Uninstall by deleting those three folders and removing the `PATH` entry under
**System Properties → Environment Variables → User variables**.
