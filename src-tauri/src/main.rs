// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser as _;
use cenno_lib::cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Ask { title, body, timeout }) => {
            run_ask(title, body, timeout);
        }
        None => {
            // No subcommand → launch the Tauri GUI, honoring --tray.
            if cli.mcp_stdio {
                eprintln!("not yet implemented (Task 9)");
                std::process::exit(1);
            }
            cenno_lib::run(cli.tray);
        }
    }
}

fn run_ask(title: String, body: String, timeout: u64) {
    let sock_path = cenno_lib::mcp::socket_path();

    if !sock_path.exists() {
        eprintln!("cenno is not running — start it or use 'cenno --mcp-stdio'");
        std::process::exit(1);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let result = rt.block_on(cenno_lib::mcp::client::call_ask_user(
        &sock_path,
        serde_json::json!({
            "title": title,
            "body_md": body,
            "timeout_s": timeout,
        }),
    ));

    match result {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()));
            // Determine exit code from shape:
            // TimedOut shape: {"answered": false, "prompt_id": "..."}
            // Answered shape: {"answer": ..., "via": ..., "elapsed_s": ...}
            if value.get("answered").and_then(|v| v.as_bool()) == Some(false) {
                std::process::exit(2);
            }
            // Answered — exit 0 (default)
        }
        Err(e) => {
            // Connection refused or similar → app likely not running
            let msg = e.to_string();
            if msg.contains("Connection refused") || msg.contains("No such file") {
                eprintln!("cenno is not running — start it or use 'cenno --mcp-stdio'");
                std::process::exit(1);
            }
            eprintln!("cenno ask: error: {e}");
            std::process::exit(1);
        }
    }
}
