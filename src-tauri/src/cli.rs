use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "cenno", about = "Agent UX runtime — agents ask, you answer")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Run headless with surfaces ready (no main window shown until a prompt arrives)
    #[arg(long)]
    pub tray: bool,

    /// Bridge stdin/stdout to the MCP socket, launching the app if needed
    #[arg(long)]
    pub mcp_stdio: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Ask the user a question and print the JSON result
    Ask {
        /// The question shown to the user
        title: String,
        /// Markdown body shown under the title
        #[arg(long, default_value = "")]
        body: String,
        /// Optional short spoken summary for sound-out (voice-out reads this
        /// instead of the body when set)
        #[arg(long, default_value = "")]
        say: String,
        /// Seconds to wait for an answer
        #[arg(long, default_value = "120")]
        timeout: u64,
    },
    /// Export prompt history to stdout (headless — no app needed)
    Export {
        /// Output format
        #[arg(long, value_enum, default_value = "json")]
        format: ExportFormat,
        /// Only include rows at or after this time (RFC3339 or YYYY-MM-DD → midnight UTC)
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
}

/// Parse a `--since` string: RFC3339 timestamp or bare `YYYY-MM-DD` date
/// (interpreted as midnight UTC, inclusive).  Returns a user-facing error
/// message on failure.
pub fn parse_since(s: &str) -> Result<DateTime<Utc>, String> {
    // Try RFC3339 first.
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Try bare date YYYY-MM-DD → midnight UTC.
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let midnight = date
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid");
        return Ok(Utc.from_utc_datetime(&midnight));
    }
    Err(format!(
        "invalid --since value {s:?}: expected RFC3339 (e.g. 2025-01-15T09:00:00Z) \
         or a date (e.g. 2025-01-15)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_ask() {
        let cli =
            Cli::try_parse_from(["cenno", "ask", "How deep?", "--timeout", "30"]).unwrap();
        match cli.command {
            Some(Command::Ask { title, timeout, .. }) => {
                assert_eq!(title, "How deep?");
                assert_eq!(timeout, 30);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn no_args_means_gui() {
        assert!(Cli::try_parse_from(["cenno"]).unwrap().command.is_none());
    }

    #[test]
    fn parses_export_defaults() {
        let cli = Cli::try_parse_from(["cenno", "export"]).unwrap();
        match cli.command {
            Some(Command::Export { format, since }) => {
                assert!(matches!(format, ExportFormat::Json));
                assert!(since.is_none());
            }
            _ => panic!("expected Export command"),
        }
    }

    #[test]
    fn parses_export_csv_with_since_date() {
        let cli =
            Cli::try_parse_from(["cenno", "export", "--format", "csv", "--since", "2025-06-01"])
                .unwrap();
        match cli.command {
            Some(Command::Export { format, since }) => {
                assert!(matches!(format, ExportFormat::Csv));
                assert_eq!(since.as_deref(), Some("2025-06-01"));
            }
            _ => panic!("expected Export command"),
        }
    }

    #[test]
    fn parse_since_bare_date_is_midnight_utc() {
        let dt = parse_since("2025-06-10").unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-06-10T00:00:00+00:00");
    }

    #[test]
    fn parse_since_rfc3339() {
        use chrono::Timelike as _;
        let dt = parse_since("2025-06-10T15:30:00Z").unwrap();
        assert_eq!(dt.hour(), 15);
        assert_eq!(dt.minute(), 30);
    }

    #[test]
    fn parse_since_invalid_gives_error() {
        assert!(parse_since("not-a-date").is_err());
        assert!(parse_since("2025/06/10").is_err());
    }
}
