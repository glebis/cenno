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
        Some(Command::Export { format, since }) => {
            run_export(format, since);
        }
        None => {
            // No subcommand → either bridge stdio to the MCP socket, or
            // launch the Tauri GUI (honoring --tray).
            if cli.mcp_stdio {
                run_mcp_stdio();
            } else {
                cenno_lib::run();
            }
        }
    }
}

/// `cenno export`: dump history to stdout as JSON or CSV.  Headless — opens
/// the DB directly without requiring the app to be running.
fn run_export(format: cenno_lib::cli::ExportFormat, since_str: Option<String>) -> ! {
    // Resolve --since if provided.
    let since = match since_str {
        Some(ref s) => match cenno_lib::cli::parse_since(s) {
            Ok(dt) => Some(dt),
            Err(msg) => {
                eprintln!("cenno export: {msg}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    // Locate the DB.
    let db_path = cenno_lib::mcp::data_dir().join("cenno.db");
    if !db_path.exists() {
        eprintln!("cenno export: no history yet — the cenno app hasn't recorded any prompts");
        std::process::exit(1);
    }

    // Open the DB (read-only is fine; open() creates tables idempotently).
    let db = match cenno_lib::db::Db::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cenno export: failed to open history DB: {e}");
            std::process::exit(1);
        }
    };

    let rows = match db.export_rows(since) {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("cenno export: query failed: {e}");
            std::process::exit(1);
        }
    };

    match format {
        cenno_lib::cli::ExportFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).expect("Vec<Value> is always serializable")
            );
        }
        cenno_lib::cli::ExportFormat::Csv => {
            // Fixed column order matching the schema.
            const COLUMNS: &[&str] = &[
                "id", "prompt_id", "title", "body_md", "input_kind", "flow", "urgency",
                "status", "answer", "via", "elapsed_s", "created_at", "resolved_at",
            ];
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record(COLUMNS).expect("csv header write failed");
            for row in &rows {
                let record: Vec<String> = COLUMNS
                    .iter()
                    .map(|col| match &row[col] {
                        serde_json::Value::Null => String::new(),
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect();
                wtr.write_record(&record).expect("csv row write failed");
            }
            wtr.flush().expect("csv flush failed");
        }
    }

    std::process::exit(0);
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
