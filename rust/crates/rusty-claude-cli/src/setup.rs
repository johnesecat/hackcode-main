use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::platform;

const GREEN: &str = "\x1b[38;2;0;255;65m";
const DIM: &str = "\x1b[90m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[91m";

const BANNER: &str = "\x1b[38;2;0;255;65m
 ██╗  ██╗ █████╗  ██████╗██╗  ██╗ ██████╗ ██████╗ ██████╗ ███████╗
 ██║  ██║██╔══██╗██╔════╝██║ ██╔╝██╔════╝██╔═══██╗██╔══██╗██╔════╝
 ███████║███████║██║     █████╔╝ ██║     ██║   ██║██║  ██║█████╗
 ██╔══██║██╔══██║██║     ██╔═██╗ ██║     ██║   ██║██║  ██║██╔══╝
 ██║  ██║██║  ██║╚██████╗██║  ██╗╚██████╗╚██████╔╝██████╔╝███████╗
 ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝
\x1b[0m";

struct Model {
    key: &'static str,
    id: &'static str,
    name: &'static str,
    size: &'static str,
    min_ram: u64,
}

const MODELS: &[Model] = &[
    Model { key: "a", id: "tripolskypetr/qwen3.5-uncensored-aggressive:4b",   name: "Qwen3.5-4B Uncensored",             size: "~3GB",  min_ram: 4  },
    Model { key: "b", id: "tripolskypetr/qwen3.5-uncensored-aggressive:8b",   name: "Qwen3.5-8B Uncensored",             size: "~5GB",  min_ram: 8  },
    Model { key: "c", id: "tripolskypetr/qwen3.5-uncensored-aggressive:14b",  name: "Qwen3.5-14B Uncensored",            size: "~9GB",  min_ram: 12 },
    Model { key: "d", id: "tripolskypetr/qwen3.5-uncensored-aggressive:32b",  name: "Qwen3.5-32B Uncensored",            size: "~19GB", min_ram: 24 },
    Model { key: "e", id: "tripolskypetr/qwen3.5-uncensored-aggressive:35b",  name: "Qwen3.5-35B-A3B Uncensored (MoE)",  size: "~21GB", min_ram: 24 },
    Model { key: "f", id: "vaultbox/qwen3.5-uncensored:35b",                  name: "Qwen3.5-35B Uncensored + Vision",   size: "~23GB", min_ram: 32 },
];

const BREW_TOOLS: &[&str] = &[
    "nmap", "masscan", "whois", "gobuster", "nikto", "hydra",
    "john-jumbo", "hashcat", "sqlmap", "whatweb", "ffuf",
    "binwalk", "exiftool", "socat", "netcat",
];

const PIP_TOOLS: &[&str] = &["impacket", "wpscan", "dnsrecon"];

/// Tools we know are installable per-user without admin via `winget` or
/// `scoop` on Windows 10/11. Anything not on this list is suggested as a
/// download link instead of an automated installer line, so the wizard never
/// needs elevation.
const WINGET_TOOLS: &[(&str, &str)] = &[
    ("nmap", "Insecure.Nmap"),
    ("openssl", "ShiningLight.OpenSSL"),
    ("curl", "cURL.cURL"),
    ("git", "Git.Git"),
    ("python3", "Python.Python.3"),
    ("jq", "stedolan.jq"),
];

fn config_dir() -> PathBuf {
    platform::config_dir()
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

fn which(cmd: &str) -> bool {
    platform::which(cmd)
}

fn exec(cmd: &str) -> String {
    platform::exec_capture(cmd)
}

fn run_cmd(cmd: &str) -> bool {
    platform::run_interactive(cmd)
}

fn ask(prompt: &str) -> String {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    input.trim().to_string()
}

fn get_ram_gb() -> u64 {
    platform::ram_gb()
}

fn get_gpu() -> String {
    platform::gpu_label()
}

fn missing_tools(tools: &[&str]) -> Vec<String> {
    tools.iter()
        .filter(|t| {
            let binary = if **t == "john-jumbo" { "john" } else { t };
            !which(binary)
        })
        .map(|t| t.to_string())
        .collect()
}

pub fn run_setup() -> Result<(), Box<dyn std::error::Error>> {
    println!("{BANNER}");
    println!("{GREEN}[HackCode]{RESET} First-run setup\n");

    // Detect hardware
    let ram_gb = get_ram_gb();
    let gpu = get_gpu();

    println!("  {DIM}GPU:{RESET}  {gpu} ({ram_gb}GB)");
    println!("  {DIM}Platform:{RESET} {} ({})", env::consts::OS, env::consts::ARCH);
    println!();

    // Step 1: Check Ollama
    println!("{GREEN}[Step 1/3]{RESET} {BOLD}AI Backend{RESET}");
    if which("ollama") {
        println!("  Ollama {GREEN}✓{RESET} installed");
    } else {
        println!("  Ollama {RED}✗{RESET} not found");
        let answer = ask("  Install Ollama now? [Y/n] ");
        if answer.to_lowercase() != "n" {
            run_cmd(platform::ollama_install_command());
        }
        if !which("ollama") {
            println!("  {DIM}If automatic install failed: {}{RESET}", platform::ollama_install_hint());
        }
    }
    println!();

    // Step 2: Model selection
    println!("{GREEN}[Step 2/3]{RESET} {BOLD}AI Model{RESET}");
    // Pick the largest model that fits in available RAM.
    let recommended = MODELS.iter().rev().find(|m| m.min_ram <= ram_gb).unwrap_or(&MODELS[0]);
    println!("  {GREEN}Recommended:{RESET} {BOLD}{}{RESET} ({})", recommended.name, recommended.size);
    println!();
    for m in MODELS {
        let rec = if m.key == recommended.key {
            format!(" {GREEN}(recommended){RESET}")
        } else {
            String::new()
        };
        println!("  {BOLD}[{}]{RESET} {:35} {DIM}{:8} min {}GB RAM{RESET}{rec}", m.key, m.name, m.size, m.min_ram);
    }
    println!("  {BOLD}[h]{RESET} Pull any model from HuggingFace");
    println!("  {BOLD}[s]{RESET} Skip model download");
    println!();

    let choice = ask(&format!("  {GREEN}>{RESET} "));
    let choice = if choice.is_empty() { recommended.key.to_string() } else { choice };

    let model_id: String;

    if choice == "h" {
        println!();
        println!("  {BOLD}HuggingFace Model Import{RESET}");
        println!("  {DIM}Paste a HuggingFace model URL or repo ID.{RESET}");
        println!("  {DIM}Examples:{RESET}");
        println!("    {DIM}https://huggingface.co/dealignai/Gemma-4-31B-JANG_4M-CRACK{RESET}");
        println!("    {DIM}bartowski/Qwen3-30B-A3B-GGUF{RESET}");
        println!("    {DIM}unsloth/DeepSeek-R1-0528-GGUF{RESET}");
        println!();
        let hf_input = ask(&format!("  {GREEN}HuggingFace model>{RESET} "));
        // Strip full URL to repo ID: huggingface.co/user/model -> user/model
        let hf_repo = hf_input
            .trim()
            .replace("https://huggingface.co/", "")
            .replace("http://huggingface.co/", "")
            .trim_end_matches('/')
            .to_string();

        if hf_repo.is_empty() {
            println!("  {RED}No model specified, using recommended model.{RESET}");
            model_id = recommended.id.to_string();
        } else {
            let hf_ollama_id = format!("hf.co/{hf_repo}");
            println!("\n  Pulling {BOLD}{hf_ollama_id}{RESET} from HuggingFace...");
            println!("  {DIM}This may take a while depending on model size.{RESET}");
            run_cmd(&format!("ollama pull \"{hf_ollama_id}\""));
            model_id = hf_ollama_id;
        }
    } else {
        model_id = MODELS.iter()
            .find(|m| m.key == choice)
            .map(|m| m.id.to_string())
            .unwrap_or_else(|| recommended.id.to_string());
    };

    if choice != "s" && choice != "h" && which("ollama") {
        println!("\n  Pulling {BOLD}{model_id}{RESET}...");
        run_cmd(&format!("ollama pull \"{model_id}\""));

        // Create hackcode-uncensored alias with native Qwen3.5 renderer
        let modelfile = format!(
            "FROM {model_id}\nRENDERER qwen3.5\nPARSER qwen3.5\nPARAMETER stop \"<|im_start|>\"\nPARAMETER stop \"<|im_end|>\"\nPARAMETER stop \"<|endoftext|>\"\nPARAMETER temperature 0.7\nPARAMETER num_ctx 32768\n"
        );
        let modelfile_path = config_dir().join("Modelfile");
        let _ = fs::create_dir_all(config_dir());
        let _ = fs::write(&modelfile_path, &modelfile);
        run_cmd(&format!("ollama create hackcode-uncensored -f \"{}\"", modelfile_path.display()));
        println!("  {GREEN}✓{RESET} Model ready as {BOLD}hackcode-uncensored{RESET}");
    }

    // For HuggingFace models, create alias with the pulled model
    if choice == "h" && which("ollama") && !model_id.is_empty() {
        let modelfile = format!(
            "FROM {model_id}\nPARAMETER temperature 0.7\nPARAMETER num_ctx 32768\n"
        );
        let modelfile_path = config_dir().join("Modelfile");
        let _ = fs::create_dir_all(config_dir());
        let _ = fs::write(&modelfile_path, &modelfile);
        run_cmd(&format!("ollama create hackcode-uncensored -f \"{}\"", modelfile_path.display()));
        println!("  {GREEN}✓{RESET} Model ready as {BOLD}hackcode-uncensored{RESET}");
    }

    let model_id = "hackcode-uncensored";
    println!();

    // Step 3: Security tools
    println!("{GREEN}[Step 3/3]{RESET} {BOLD}Security Tools{RESET}");

    let is_macos = cfg!(target_os = "macos");
    let is_windows = cfg!(target_os = "windows");

    if is_macos && which("brew") {
        let missing = missing_tools(BREW_TOOLS);
        if missing.is_empty() {
            println!("  {GREEN}✓{RESET} All Homebrew tools installed");
        } else {
            println!("\n  Missing: {}", missing.join(", "));
            println!("  {DIM}Command: brew install {}{RESET}", missing.join(" "));
            let answer = ask(&format!("\n  Install {} tools via Homebrew? [Y/n] ", missing.len()));
            if answer.to_lowercase() != "n" {
                for (i, pkg) in missing.iter().enumerate() {
                    println!("  {GREEN}[{}/{}]{RESET} Installing {BOLD}{pkg}{RESET}...", i + 1, missing.len());
                    run_cmd(&format!("brew install {pkg}"));
                }
            }
        }

        if which("pip3") {
            let missing_pip = missing_tools(PIP_TOOLS);
            if !missing_pip.is_empty() {
                println!("\n  Missing pip tools: {}", missing_pip.join(", "));
                let answer = ask("  Install via pip? [Y/n] ");
                if answer.to_lowercase() != "n" {
                    run_cmd(&format!("pip3 install {}", missing_pip.join(" ")));
                }
            }
        }
    } else if is_windows {
        // Windows: prefer winget (ships with Win10/11, user scope, no admin).
        // We don't try to install everything — many pentest tools (sqlmap,
        // wpscan, hydra, ...) ship as portable scripts or run best inside
        // Kali on WSL. We surface clear hints instead of failing silently.
        let has_winget = which("winget");
        let has_scoop = which("scoop");
        if has_winget || has_scoop {
            let mut missing_pkgs: Vec<(&&str, &&str)> = WINGET_TOOLS
                .iter()
                .map(|(bin, pkg)| (bin, pkg))
                .filter(|(bin, _)| !which(bin))
                .collect();
            if missing_pkgs.is_empty() {
                println!("  {GREEN}✓{RESET} Common tools installed");
            } else {
                println!("\n  Missing tools available without admin: {}",
                    missing_pkgs.iter().map(|(b, _)| **b).collect::<Vec<_>>().join(", "));
                let cmd = if has_winget {
                    let pkgs: Vec<String> = missing_pkgs
                        .drain(..)
                        .map(|(_, pkg)| format!("winget install --id {pkg} --accept-source-agreements --accept-package-agreements --silent --scope user"))
                        .collect();
                    pkgs.join("; ")
                } else {
                    let pkgs: Vec<String> = missing_pkgs
                        .drain(..)
                        .map(|(bin, _)| format!("scoop install {bin}"))
                        .collect();
                    pkgs.join("; ")
                };
                let answer = ask("  Install via per-user package manager? [Y/n] ");
                if answer.to_lowercase() != "n" {
                    run_cmd(&cmd);
                }
            }
        } else {
            println!(
                "  {DIM}Install winget (ships with Windows 10/11) or scoop ({BOLD}https://scoop.sh{RESET}{DIM}) to auto-install common security tools per-user (no admin needed).{RESET}"
            );
        }
        println!(
            "  {DIM}For the full Kali toolchain on Windows, install Kali via WSL:{RESET}"
        );
        println!(
            "  {BOLD}wsl --install -d kali-linux{RESET}"
        );
    } else if which("apt") {
        println!("  {DIM}Install tools with: sudo apt install nmap gobuster nikto hydra sqlmap ...{RESET}");
    }

    // Save config
    let config = format!(
        "{{\n  \"model\": \"{model_id}\",\n  \"baseURL\": \"http://localhost:11434/v1\"\n}}\n"
    );
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    fs::write(config_path(), &config)?;

    let config_path_display = config_path().display().to_string();

    println!("\n{GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");
    println!("{GREEN}[HackCode]{RESET} Setup complete!\n");
    println!("  {DIM}Config:{RESET}  {config_path_display}");
    println!("  {DIM}Model:{RESET}   {model_id}\n");
    println!("  {BOLD}hackcode{RESET}          {DIM}# Start hacking{RESET}");
    println!("  {BOLD}hackcode --help{RESET}   {DIM}# Show all commands{RESET}");
    println!("  {BOLD}hackcode --setup{RESET}  {DIM}# Re-run this setup{RESET}");
    println!("{GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}\n");

    Ok(())
}
