// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser as _;
use cenno_lib::cli::{Cli, Command, SecretCommand};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Ask { title, body, say, timeout, muted }) => {
            run_ask(title, body, say, timeout, muted);
        }
        Some(Command::Export { format, since }) => {
            run_export(format, since);
        }
        Some(Command::Secret(cmd)) => {
            run_secret(cmd);
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
                "status", "answer", "via", "elapsed_s", "created_at", "resolved_at", "agent",
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

/// `cenno secret …`: list / get / set / delete / run encrypted credentials.
///
/// Deliberately headless — no app required. `run` is the interesting one: it
/// injects decrypted values into a child process's environment so an agent can
/// USE a credential without the value ever appearing in its context or in the
/// history database.
fn run_secret(cmd: SecretCommand) -> ! {
    let cfg = cenno_lib::config::Config::load().secrets;

    match cmd {
        SecretCommand::List { json } => match cenno_lib::secret::list(&cfg) {
            Ok(items) if json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&items).expect("SecretMeta is serializable")
                );
            }
            Ok(items) if items.is_empty() => {
                eprintln!("no secrets stored in ~/.cenno/secrets");
            }
            Ok(items) => {
                for m in items {
                    let mut line = m.name;
                    if let Some(f) = m.format {
                        line.push_str(&format!("\t{f}"));
                    }
                    if let Some(l) = m.label {
                        line.push_str(&format!("\t{l}"));
                    }
                    if let Some(u) = m.updated_at {
                        line.push_str(&format!("\t{u}"));
                    }
                    println!("{line}");
                }
            }
            Err(e) => {
                eprintln!("cenno secret list: {e}");
                std::process::exit(1);
            }
        },
        SecretCommand::Get { name } => match cenno_lib::secret::get(&cfg, &name) {
            // No trailing newline: the canonical use is `$(cenno secret get x)`.
            Ok(value) => {
                use std::io::Write as _;
                let _ = std::io::stdout().write_all(value.as_bytes());
            }
            Err(e) => {
                eprintln!("cenno secret get: {e}");
                std::process::exit(1);
            }
        },
        SecretCommand::Set { name, format, label } => {
            // Read the value from stdin, never from argv: argv is world-readable
            // via `ps` and lands in shell history.
            let value = match read_secret_stdin(&name) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("cenno secret set: {e}");
                    std::process::exit(1);
                }
            };
            match cenno_lib::secret::store(&cfg, &name, &value, format.as_deref(), label.as_deref())
            {
                Ok(meta) => {
                    // Warn-but-store on a format mismatch, matching the panel:
                    // never block a legitimate, rotated or proxied key.
                    if let (Some(fmt), Ok(false)) = (
                        meta.format.as_deref(),
                        cenno_lib::secret_formats::validate(
                            meta.format.as_deref().unwrap_or_default(),
                            &value,
                        ),
                    ) {
                        eprintln!(
                            "cenno secret set: value does not look like a {fmt} — stored anyway"
                        );
                    }
                    eprintln!("stored {name} ({})", cenno_lib::secret::hint(&value));
                }
                Err(e) => {
                    eprintln!("cenno secret set: {e}");
                    std::process::exit(1);
                }
            }
        }
        SecretCommand::Delete { name } => match cenno_lib::secret::delete(&cfg, &name) {
            Ok(true) => eprintln!("deleted {name}"),
            Ok(false) => {
                eprintln!("cenno secret delete: no secret named {name:?}");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("cenno secret delete: {e}");
                std::process::exit(1);
            }
        },
        SecretCommand::Run {
            name,
            with,
            command,
        } => {
            let (program, args) = command.split_first().expect("clap requires a command");
            let mut cmd = std::process::Command::new(program);
            cmd.args(args);
            for name in std::iter::once(&name).chain(with.iter()) {
                let value = match cenno_lib::secret::get(&cfg, name) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("cenno secret run: {e}");
                        std::process::exit(1);
                    }
                };
                cmd.env(env_var_for(name), value);
            }
            // Inherit stdio; propagate the child's exit code so this is a
            // drop-in prefix for any command.
            let status = match cmd.status() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cenno secret run: could not run {program}: {e}");
                    std::process::exit(127);
                }
            };
            std::process::exit(status.code().unwrap_or(1));
        }
    }

    std::process::exit(0);
}

/// Map a secret name to the env var it becomes under `cenno secret run`:
/// `openai_api_key` → `CENNO_SECRET_OPENAI_API_KEY`.
fn env_var_for(name: &str) -> String {
    let upper: String = name
        .chars()
        .map(|c| match c {
            'a'..='z' => c.to_ascii_uppercase(),
            '0'..='9' => c,
            _ => '_',
        })
        .collect();
    format!("CENNO_SECRET_{upper}")
}

/// Read a secret value from stdin, stripped of the trailing newline a pipe or
/// `echo` adds. Refuses an empty value so `cenno secret set x < /dev/null`
/// cannot silently wipe a working credential.
fn read_secret_stdin(name: &str) -> Result<String, String> {
    use std::io::Read as _;
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("reading the value from stdin: {e}"))?;
    let value = buf.trim_end_matches(['\n', '\r']).to_string();
    if value.is_empty() {
        return Err(format!(
            "no value on stdin — pipe it in (e.g. `pbpaste | cenno secret set {name}`)"
        ));
    }
    Ok(value)
}

fn run_ask(title: String, body: String, say: String, timeout: u64, muted: bool) {
    let sock_path = cenno_lib::mcp::socket_path();

    if !sock_path.exists() {
        eprintln!("cenno is not running — start it or use 'cenno --mcp-stdio'");
        std::process::exit(1);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let mut payload = serde_json::json!({
        "title": title,
        "body_md": body,
        "timeout_s": timeout,
    });
    if !say.is_empty() {
        payload["say"] = serde_json::Value::String(say);
    }
    if muted {
        payload["muted"] = serde_json::Value::Bool(true);
    }

    let result = rt.block_on(cenno_lib::mcp::client::call_ask_user(&sock_path, payload));

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
                Ok(AskResponse::SecretStored { .. }) => {} // exit 0
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
