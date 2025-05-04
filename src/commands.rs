//! This module defines the command-line interface for the application using `clap`.
//!
//! It provides a `Cli` struct that represents the parsed command-line arguments,
//! and an `Commands` enum that represents the available subcommands and their
//! options.
//!
//! # Examples
//!
//! Parsing command-line arguments:
//!
//! ```no_run
//! use clap::{Parser, Subcommand};
//! use awful_aj::commands::{Cli, Commands};
//!
//! let cli = Cli::parse();
//! // TODO
//! //match cli.command {
//! //    Commands::Ask { question } => {
//!         // Handle the 'ask' subcommand
//! //    }
//! //    Commands::Init => {
//!         // Handle the 'init' subcommand
//! //    }
//! //}
//! ```

use clap::{Parser, Subcommand};

/// Represents the parsed command-line arguments.
///
/// This struct is constructed by parsing the command-line arguments using `clap`.
/// It contains a `command` field that holds the parsed subcommand and its options.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, propagate_version = true, color = clap::ColorChoice::Always)]
pub struct Cli {
    /// The parsed subcommand and its options.
    #[command(subcommand)]
    pub command: Commands,
}

/// Represents the available subcommands and their options.
///
/// Each variant of this enum corresponds to a subcommand that the user can invoke
/// from the command line, along with any options specific to that subcommand.
#[derive(Subcommand, Debug)]
#[command(about, long_about = None, color = clap::ColorChoice::Always)]
pub enum Commands {
    /// The 'ask' subcommand, which takes an optional question as an argument.
    ///
    /// If the question is not provided on the command line, a default question
    /// will be used.
    #[clap(name = "ask", alias = "a")]
    Ask {
        /// The question to be asked. If not provided, a default question is used.
        question: Option<String>,

        #[arg(name = "template", short = 't')]
        template: Option<String>,

        #[arg(name = "session", short = 's')]
        session: Option<String>,
    },

    /// The 'interactive' subcommand, which can have an optional name for the conversation.
    ///
    /// This subcommand can be invoked with either 'i' or 'interactive'.
    #[clap(name = "interactive", alias = "i")]
    Interactive {
        #[arg(name = "template", short = 't')]
        template: Option<String>,

        #[arg(name = "session", short = 's')]
        session: Option<String>,
    },

    /// The 'init' subcommand, which takes no arguments and is used for initialization.
    ///
    /// When invoked, this subcommand performs setup and initialization tasks, such
    /// as creating necessary directories and files.
    Init,
}
