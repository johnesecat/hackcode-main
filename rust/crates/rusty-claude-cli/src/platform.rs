//! Cross-platform helpers for the HackCode CLI surface.
//!
//! Linux/macOS rely on POSIX-style paths and `bash`/`sh` to run setup commands;
//! Windows 10/11 has neither by default. This module isolates everything that
//! changes between platforms so the rest of the CLI (`setup.rs`, `scanner.rs`,
//! `main.rs`) can stay platform-agnostic.
//!
//! Goals:
//! * Native PowerShell support on Windows (no admin rights, no WSL, no Cygwin).
//! * Continue to use `/bin/bash` on macOS / Linux exactly as before.
//! * Install destinations and config locations follow each platform's
//!   per-user, isolated-environment-friendly conventions:
//!     * Unix:   `~/.local/bin`,  `~/.config/hackcode`
//!     * Windows: `%LOCALAPPDATA%\Programs\HackCode`, `%APPDATA%\hackcode`

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Per-user home directory. Unix uses `$HOME`; Windows uses `%USERPROFILE%`.
/// Falls back to the OS temp dir if neither is set so we never panic.
#[must_use]
pub fn home_dir() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(profile) = env::var("USERPROFILE") {
        if !profile.is_empty() {
            return PathBuf::from(profile);
        }
    }
    env::temp_dir()
}

/// Per-user HackCode config directory.
///
/// * Unix:   `$HOME/.config/hackcode`
/// * Windows: `%APPDATA%\hackcode` (falls back to `%USERPROFILE%\.config\hackcode`)
#[must_use]
pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            if !appdata.is_empty() {
                return PathBuf::from(appdata).join("hackcode");
            }
        }
        return home_dir().join(".config").join("hackcode");
    }
    #[cfg(not(windows))]
    {
        home_dir().join(".config").join("hackcode")
    }
}

/// Per-user `bin` directory where we install the `hackcode` executable.
///
/// * Unix:   `$HOME/.local/bin`
/// * Windows: `%LOCALAPPDATA%\Programs\HackCode` (no admin rights required)
#[must_use]
pub fn local_bin_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            if !local.is_empty() {
                return PathBuf::from(local).join("Programs").join("HackCode");
            }
        }
        return home_dir()
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("HackCode");
    }
    #[cfg(not(windows))]
    {
        home_dir().join(".local").join("bin")
    }
}

/// File name of the installed binary on the active platform.
#[must_use]
pub const fn binary_exe_name() -> &'static str {
    if cfg!(windows) {
        "hackcode.exe"
    } else {
        "hackcode"
    }
}

/// Where source checkouts of the repo are cloned for `--update`.
#[must_use]
pub fn src_dir() -> PathBuf {
    home_dir().join(".hackcode-src")
}

/// PATH augmented with the well-known per-platform install locations of
/// Ollama and any HackCode-installed binaries so `which` / spawned processes
/// can find them even if the user has not added them to their shell rc yet.
#[must_use]
pub fn extra_path() -> String {
    let current = env::var("PATH").unwrap_or_default();
    #[cfg(windows)]
    {
        // Common Ollama install locations on Windows 10/11 plus our own
        // per-user bin dir and `cargo`'s default per-user install dir
        // (rustup installs `cargo.exe` under `%USERPROFILE%\.cargo\bin` and
        // only injects it into the user `PATH` on next logon — so freshly
        // installed cargo wouldn't otherwise be reachable from the running
        // shell). We use ';' as the PATH separator on Windows.
        let local = env::var("LOCALAPPDATA").unwrap_or_default();
        let programfiles = env::var("ProgramFiles").unwrap_or_default();
        let userprofile = env::var("USERPROFILE")
            .ok()
            .or_else(|| env::var("HOME").ok())
            .unwrap_or_default();
        let mut parts = Vec::new();
        if !local.is_empty() {
            parts.push(format!("{local}\\Programs\\Ollama"));
            parts.push(format!("{local}\\Programs\\HackCode"));
        }
        if !programfiles.is_empty() {
            parts.push(format!("{programfiles}\\Ollama"));
        }
        if !userprofile.is_empty() {
            parts.push(format!("{userprofile}\\.cargo\\bin"));
        }
        parts.push(current);
        return parts.join(";");
    }
    #[cfg(not(windows))]
    {
        // macOS Homebrew + Linux + Ollama.app on macOS.
        format!(
            "/opt/homebrew/bin:/usr/local/bin:/usr/sbin:/Applications/Ollama.app/Contents/Resources:{current}"
        )
    }
}

/// Returns true if `cmd` resolves to an executable on `PATH` (with our
/// platform-specific extra path applied).
#[must_use]
pub fn which(cmd: &str) -> bool {
    #[cfg(windows)]
    {
        // `where.exe` is the Windows analogue of `which`. It ships with
        // Windows 10/11 by default and works in any PowerShell or cmd.exe
        // session without admin rights.
        Command::new("where")
            .arg(cmd)
            .env("PATH", extra_path())
            .output()
            .map_or(false, |o| o.status.success())
    }
    #[cfg(not(windows))]
    {
        Command::new("which")
            .arg(cmd)
            .env("PATH", extra_path())
            .output()
            .map_or(false, |o| o.status.success())
    }
}

/// Build a `Command` that invokes the right shell for the current platform
/// and runs `script` non-interactively. The caller is responsible for
/// configuring stdio.
#[must_use]
pub fn shell_command(script: &str) -> Command {
    #[cfg(windows)]
    {
        // Use PowerShell on Windows. `-NoProfile` keeps things hermetic so
        // user profile scripts don't change behavior, and `-Command -` reads
        // the script from `args` so we can pass an arbitrary string.
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ]);
        cmd.env("PATH", extra_path());
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("/bin/bash");
        cmd.args(["-c", script]);
        cmd.env("PATH", extra_path());
        cmd
    }
}

/// Capture stdout of `cmd` after running it through the platform shell.
#[must_use]
pub fn exec_capture(cmd: &str) -> String {
    shell_command(cmd)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Run `cmd` in the platform shell with stdio inherited (so the user can
/// see progress and respond to prompts). Returns true on a zero exit code.
#[must_use]
pub fn run_interactive(cmd: &str) -> bool {
    let mut command = shell_command(cmd);
    command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    #[cfg(not(windows))]
    {
        command.env("HOMEBREW_NO_AUTO_UPDATE", "1");
    }
    command.status().map_or(false, |s| s.success())
}

/// Spawn `ollama serve` in the background. Returns immediately; we poll the
/// TCP port from the caller to confirm readiness.
pub fn spawn_ollama_serve() {
    #[cfg(windows)]
    {
        // `Start-Process -WindowStyle Hidden` detaches the process so it
        // keeps running after the parent exits, and avoids opening a
        // visible PowerShell window. Use `-FilePath ollama` so PowerShell
        // resolves the executable via PATH (including our extra path).
        let _ = shell_command(
            "Start-Process -FilePath 'ollama' -ArgumentList 'serve' -WindowStyle Hidden",
        )
        .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = shell_command("ollama serve &>/dev/null &").spawn();
    }
}

/// Total physical memory in GiB.
#[must_use]
pub fn ram_gb() -> u64 {
    #[cfg(target_os = "macos")]
    {
        let raw = exec_capture("sysctl -n hw.memsize 2>/dev/null");
        return raw.parse::<u64>().unwrap_or(0) / 1024 / 1024 / 1024;
    }
    #[cfg(target_os = "linux")]
    {
        let raw = exec_capture("grep MemTotal /proc/meminfo 2>/dev/null | awk '{print $2}'");
        return raw.parse::<u64>().unwrap_or(0) / 1024 / 1024;
    }
    #[cfg(target_os = "windows")]
    {
        // CIM is the modern, non-deprecated path on Windows 10/11. Falls
        // back to wmic if CIM is somehow unavailable on a stripped-down
        // image. Result is bytes; convert to GiB.
        let raw =
            exec_capture("(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory");
        if let Ok(bytes) = raw.parse::<u64>() {
            return bytes / 1024 / 1024 / 1024;
        }
        let raw = exec_capture("wmic computersystem get TotalPhysicalMemory /value");
        for line in raw.lines() {
            if let Some(value) = line.trim().strip_prefix("TotalPhysicalMemory=") {
                if let Ok(bytes) = value.parse::<u64>() {
                    return bytes / 1024 / 1024 / 1024;
                }
            }
        }
        return 0;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        0
    }
}

/// Best-effort GPU / compute-device label.
#[must_use]
pub fn gpu_label() -> String {
    #[cfg(target_os = "macos")]
    {
        let cpu = exec_capture("sysctl -n machdep.cpu.brand_string 2>/dev/null");
        return if cpu.is_empty() {
            "Unknown".to_string()
        } else {
            cpu
        };
    }
    #[cfg(target_os = "linux")]
    {
        let gpu = exec_capture("nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null");
        return if gpu.is_empty() {
            "CPU only".to_string()
        } else {
            gpu
        };
    }
    #[cfg(target_os = "windows")]
    {
        // Try NVIDIA first (matches the Linux behavior), then fall back to
        // CIM to enumerate any video controller. nvidia-smi is shipped with
        // the NVIDIA driver on Windows when present.
        let gpu = exec_capture("nvidia-smi --query-gpu=name --format=csv,noheader 2>$null");
        if !gpu.is_empty() {
            return gpu;
        }
        let gpu = exec_capture(
            "(Get-CimInstance -ClassName Win32_VideoController | Select-Object -First 1).Name",
        );
        if !gpu.is_empty() {
            return gpu;
        }
        return "CPU only".to_string();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "Unknown".to_string()
    }
}

/// Human-readable hint for installing Ollama on the active platform.
#[must_use]
pub const fn ollama_install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install ollama   (or download from https://ollama.ai/download)"
    } else if cfg!(target_os = "windows") {
        // winget ships with Windows 10/11 and does not require admin for
        // user-scope installs. Scoop is the most popular no-admin alt.
        "winget install Ollama.Ollama   (or scoop install ollama, or download https://ollama.ai/download/windows)"
    } else {
        "curl -fsSL https://ollama.ai/install.sh | sh"
    }
}

/// Best-effort cross-platform script to install Ollama without prompting
/// the user, used by `setup` when `ollama` is not on `PATH`.
#[must_use]
pub const fn ollama_install_command() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        // Try winget first (ships with Windows 10/11, user scope, no admin).
        // Fall back to a direct download to %LOCALAPPDATA% if winget isn't
        // available (e.g. Windows Sandbox / restricted environments).
        "if (Get-Command winget -ErrorAction SilentlyContinue) { \
             winget install --id Ollama.Ollama --accept-source-agreements --accept-package-agreements --silent --scope user \
         } else { \
             Write-Host 'winget not found; download Ollama for Windows from https://ollama.ai/download/windows' \
         }"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "curl -fsSL https://ollama.ai/install.sh | sh"
    }
}
