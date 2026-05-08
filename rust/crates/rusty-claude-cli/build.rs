use std::env;
use std::process::Command;

fn main() {
    // Get git SHA (short hash)
    let git_sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map_or_else(|| "unknown".to_string(), |s| s.trim().to_string());

    println!("cargo:rustc-env=GIT_SHA={git_sha}");

    // TARGET is always set by Cargo during build
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=TARGET={target}");

    // Build date from SOURCE_DATE_EPOCH (reproducible builds) or current UTC date.
    // Intentionally ignoring time component to keep output deterministic within a day.
    let build_date = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| epoch.parse::<i64>().ok())
        .map(|_ts| {
            // Use SOURCE_DATE_EPOCH to derive date via chrono if available;
            // for simplicity we just use the env var as a signal and fall back
            // to build-time env. In practice CI sets this via workflow.
            std::env::var("BUILD_DATE").unwrap_or_else(|_| "unknown".to_string())
        })
        .or_else(|| std::env::var("BUILD_DATE").ok())
        .unwrap_or_else(|| {
            // Fall back to current date. The Unix `date` binary supports
            // `+%Y-%m-%d` but on Windows the built-in `date` command is
            // interactive (it prompts to set the system date). Use
            // PowerShell's `Get-Date` there so the build still succeeds in
            // a stock Windows 10/11 environment without GNU coreutils.
            #[cfg(windows)]
            let cmd = {
                let mut c = Command::new("powershell");
                c.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    "(Get-Date).ToUniversalTime().ToString('yyyy-MM-dd')",
                ]);
                c
            };
            #[cfg(not(windows))]
            let cmd = {
                let mut c = Command::new("date");
                c.args(["-u", "+%Y-%m-%d"]);
                c
            };
            let mut cmd = cmd;
            cmd.output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout).ok()
                    } else {
                        None
                    }
                })
                .map_or_else(|| "unknown".to_string(), |s| s.trim().to_string())
        });
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    // Rerun if git state changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
