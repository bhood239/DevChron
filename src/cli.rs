use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "devchron",
    about = "A beautiful TUI Pomodoro timer for Hyprland/Wayland",
    version
)]
pub struct Cli {
    /// Colour theme to use (nord, catppuccin, classic). Overrides config.
    #[arg(long, short = 't', value_name = "THEME")]
    pub theme: Option<String>,

    /// Timer profile to activate on startup (e.g. work, deep, quick). Overrides config.
    #[arg(long, short = 'p', value_name = "PROFILE")]
    pub profile: Option<String>,

    /// Disable desktop notifications for this session.
    #[arg(long)]
    pub no_notifications: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Print the current timer status as a one-liner and exit.
    ///
    /// Reads ~/.cache/devchron/status.json written by a running devchron instance.
    /// Exits with code 1 if devchron is not running.
    Status {
        /// Output raw JSON instead of a formatted string.
        #[arg(long)]
        json: bool,
    },
}
