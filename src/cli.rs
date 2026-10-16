use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "s",
    author = "joe",
    version,
    about = "Ultra-lightweight terminal session manager with file-backed buffer",
    long_about = "A middle ground between tmux and dtach. Maintains running terminal processes with zero RAM-buffered scrollback, utilizing direct file-backed logging."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Session name to attach or create (shorthand: `s main`)
    #[arg(value_name = "SESSION_NAME", index = 1)]
    pub session_name: Option<String>,

    /// Custom target shell (defaults to $SHELL or /bin/sh)
    #[arg(short = 's', long = "shell", global = true)]
    pub shell: Option<String>,

    /// Detach key combination (e.g., 'ctrl-\', 'ctrl-a', 'ctrl-]')
    #[arg(short = 'd', long = "detach-key", default_value = "ctrl-\\", global = true)]
    pub detach_key: String,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Attach to or create a session
    #[command(alias = "a")]
    Attach {
        /// Session name to attach to
        name: String,
    },

    /// List all active sessions
    #[command(alias = "ls", alias = "l")]
    List,

    /// Terminate an active session
    #[command(alias = "k")]
    Kill {
        /// Session name to terminate
        name: String,
    },

    /// View or locate the file-backed scrollback log of a session
    Log {
        /// Session name
        name: String,
        /// Print file path only
        #[arg(short, long)]
        path: bool,
    },

    /// Run background daemon server for a session (internal command)
    #[command(hide = true)]
    Server {
        /// Session name
        name: String,
    },
}

/// Parse a human-friendly detach key representation into a control byte.
pub fn parse_detach_key(s: &str) -> u8 {
    let lower = s.to_lowercase().replace(' ', "");
    match lower.as_str() {
        "ctrl-\\" | "^\\" | "ctrl-backslash" => 0x1C,
        "ctrl-a" | "^a" => 0x01,
        "ctrl-b" | "^b" => 0x02,
        "ctrl-c" | "^c" => 0x03,
        "ctrl-d" | "^d" => 0x04,
        "ctrl-e" | "^e" => 0x05,
        "ctrl-q" | "^q" => 0x11,
        "ctrl-]" | "^]" => 0x1D,
        "ctrl-^" | "^^" => 0x1E,
        "ctrl-_" | "^_" => 0x1F,
        _ => {
            if let Some(stripped) = lower.strip_prefix("0x") {
                if let Ok(b) = u8::from_str_radix(stripped, 16) {
                    return b;
                }
            }
            // Fallback to default Ctrl+\ (0x1C)
            0x1C
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_detach_key() {
        assert_eq!(parse_detach_key("ctrl-\\"), 0x1C);
        assert_eq!(parse_detach_key("^\\"), 0x1C);
        assert_eq!(parse_detach_key("ctrl-a"), 0x01);
        assert_eq!(parse_detach_key("ctrl-b"), 0x02);
        assert_eq!(parse_detach_key("ctrl-d"), 0x04);
        assert_eq!(parse_detach_key("ctrl-]"), 0x1D);
        assert_eq!(parse_detach_key("0x1b"), 0x1B);
    }
}

