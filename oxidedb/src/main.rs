//! OxideDB - A hook-first database with plugin architecture
//!
//! This is the main entry point for OxideDB. It demonstrates the integration
//! of the core architecture components using a modular, maintainable structure.

use oxidedb::{
    config::{Cli, Commands},
    commands::{CommandHandler, StartCommand, RegisterSuperuserCommand},
    Result,
};
use clap::Parser;
use tracing::Level;
use tracing_subscriber::fmt;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logging based on command-specific log level
    let log_level = match &cli.command {
        Commands::Start(args) => Level::from(args.log_level.clone()),
        Commands::RegisterSuperuser(args) => Level::from(args.log_level.clone()),
    };
    
    fmt().with_max_level(log_level).init();

    // Execute the appropriate command
    match cli.command {
        Commands::Start(args) => {
            StartCommand::execute(args).await
        }
        Commands::RegisterSuperuser(args) => {
            RegisterSuperuserCommand::execute(args).await
        }
    }
}
