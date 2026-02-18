use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(super) struct Cli {
    /// Path to configuration file
    #[arg(long, short)]
    pub config: Option<PathBuf>,

    /// Ignore all settings in the database and use only config file/defaults
    #[arg(long)]
    pub safe_mode: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub(super) enum Commands {
    /// Run the VPS node (default)
    Run,
    /// Manage node settings
    Settings {
        /// Path to settings database
        #[arg(long, default_value = "data/settings.db")]
        db: PathBuf,
        #[command(subcommand)]
        action: SettingsCommands,
    },
    /// Export node identity (nsec)
    Identity,
}

#[derive(Subcommand)]
pub(super) enum SettingsCommands {
    /// List all settings
    List,
    /// Get a specific setting
    Get { key: String },
    /// Set a setting value
    Set { key: String, value: String },
    /// Delete a setting
    Delete { key: String },
}
