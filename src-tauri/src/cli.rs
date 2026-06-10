use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cenno", about = "Agent UX runtime — agents ask, you answer")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Run headless with surfaces ready (no main window shown until a prompt arrives)
    #[arg(long)]
    pub tray: bool,

    /// Bridge stdin/stdout to the MCP socket (launching the app if needed) — implemented in Task 9
    #[arg(long)]
    pub mcp_stdio: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Ask the user a question and print the JSON result
    Ask {
        title: String,
        #[arg(long, default_value = "")]
        body: String,
        #[arg(long, default_value = "120")]
        timeout: u64,
    },
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
}
