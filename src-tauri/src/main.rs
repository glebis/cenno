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
            // No subcommand → either bridge stdio to the MCP socket, or
            // launch the Tauri GUI (honoring --tray).
            if cli.mcp_stdio {
                run_mcp_stdio();
            }
            cenno_lib::run(cli.tray);
        }
    }
}

/// `--mcp-stdio`: pump stdin/stdout to the app's MCP socket, autolaunching
/// the app if it isn't running. Exits 0 on clean EOF, 1 on error.
fn run_mcp_stdio() -> ! {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    match rt.block_on(cenno_lib::bridge::run_stdio_bridge(
        cenno_lib::mcp::socket_path(),
    )) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("cenno --mcp-stdio: {e}");
            std::process::exit(1);
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
            // Always print the raw JSON first, whatever its shape.
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
            use cenno_lib::protocol::AskResponse;
            match serde_json::from_value::<AskResponse>(value.clone()) {
                Ok(AskResponse::Answered { .. }) => {} // exit 0
                Ok(AskResponse::TimedOut { .. }) => std::process::exit(2),
                Err(e) => {
                    eprintln!("cenno ask: unrecognized response shape ({e}): {value}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            // Socket gone or nobody listening → app not running.
            let not_running = e.downcast_ref::<std::io::Error>().is_some_and(|io| {
                matches!(
                    io.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                )
            });
            if not_running {
                eprintln!("cenno is not running — start it or use 'cenno --mcp-stdio'");
            } else {
                eprintln!("cenno ask: error: {e}");
            }
            std::process::exit(1);
        }
    }
}
