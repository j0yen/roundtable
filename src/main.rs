//! `roundtable` — convene the full daily creative session.
//!
//! Orchestrates `the-lunch`, `vicious-circle`, and `conning-tower` into a
//! single command that runs the complete chain:
//!
//! ```text
//! the-lunch lunch → vicious-circle record (per artifact) → conning-tower compose + syndicate
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod session;
mod table;
mod ledger;

/// Convene the full daily creative session.
#[derive(Debug, Parser)]
#[command(name = "roundtable", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the full daily session chain.
    Session(session::SessionArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Session(args) => session::run(args),
    }
}
