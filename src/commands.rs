//! # Command-line interface
//!
//! Declarative CLI for the Awful Jade application built with [`clap`](https://docs.rs/clap).
//!
//! The CLI exposes three subcommands:
//!
//! - [`ask`](Commands::Ask): Ask a single question and print the model's answer.
//! - [`interactive`](Commands::Interactive): Start a live REPL-style chat session.
//! - [`init`](Commands::Init): Create default config and template files under the app's
//!   platform-specific config directory.
//!
//! ## Quick examples
//!
//! **Ask with defaults**
//! ```no_run
//! use awful_aj::commands::Cli;
//! use clap::Parser;
//! let cli = Cli::parse();
//! // hand off to your app's dispatcher
//! ```
//!
//! **Ask with a specific template and session**
//! ```text
//! aj ask -t simple_question -s default "What is HNSW?"
//! ```
//!
//! **Interactive mode**
//! ```text
//! aj interactive
//! ```
//!
//! ## Notes
//! - Colors are enabled by default in help output (see `ColorChoice::Always`).

use clap::{Parser, Subcommand};

/// Top-level CLI parser.
///
/// This struct is produced by [`clap::Parser::parse`] and contains
/// exactly one selected subcommand in [`Cli::command`].
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Awful Jade – a CLI for local LLM tinkering with memories, templates, and vibes.",
    long_about = None,
    propagate_version = true,
    color = clap::ColorChoice::Always
)]
pub struct Cli {
    /// Subcommand to execute (e.g., `ask`, `interactive`, `init`).
    #[command(subcommand)]
    pub command: Commands,
}

/// All supported subcommands.
///
/// See each variant's field docs for the available options.
#[derive(Subcommand, Debug)]
#[command(about, long_about = None, color = clap::ColorChoice::Always)]
pub enum Commands {
    /// Ask a single question and print the assistant's response.
    ///
    /// If no `question` is provided, the application supplies a default prompt.
    ///
    /// Aliases: `a`
    #[clap(name = "ask", alias = "a")]
    Ask {
        /// The question to ask. When omitted, a default question is used.
        question: Option<String>,

        /// Name of the chat template to load (e.g., `simple_question`).
        ///
        /// Templates live under the app's config directory, usually at:
        /// - macOS: `~/Library/Application Support/com.awful-sec.aj/templates/`
        /// - Linux: `~/.config/aj/templates/`
        /// - Windows: `%APPDATA%\\com.awful-sec\\aj\\templates\\`
        #[arg(name = "template", short = 't')]
        template: Option<String>,

        /// Session name. When set, messages are persisted under this conversation.
        ///
        /// Using a session enables retrieval-augmented context from prior turns.
        #[arg(name = "session", short = 's')]
        session: Option<String>,

        /// Force one-shot mode, ignoring any session configured in config.yaml.
        ///
        /// When this flag is set, the prompt will be treated as standalone
        /// with no memory or session tracking, even if a session_name is
        /// configured in the config file.
        #[arg(long)]
        one_shot: bool,
    },

    /// Start an interactive REPL-style conversation.
    ///
    /// Prints streaming assistant output (when enabled) and persists messages
    /// if a session name is configured by the application.
    ///
    /// Aliases: `i`
    #[clap(name = "interactive", alias = "i")]
    Interactive {
        /// Name of the chat template to load (e.g., `simple_question`).
        #[arg(name = "template", short = 't')]
        template: Option<String>,

        /// Session name for the conversation.
        #[arg(name = "session", short = 's')]
        session: Option<String>,
    },

    /// Initialize configuration and default templates in the platform config directory.
    ///
    /// Creates the config file and a minimal template set if they don't exist yet.
    Init {
        /// Overwrite existing files (config, templates, database).
        ///
        /// By default, init will skip files that already exist. Use this flag
        /// to force overwriting all files.
        #[arg(long)]
        overwrite: bool,
    },

    /// Reset the database to a pristine state.
    ///
    /// This command drops all sessions, messages, and configurations from the database,
    /// then recreates the schema. Use this to start fresh with a clean database.
    ///
    /// Aliases: `r`
    #[clap(name = "reset", alias = "r")]
    Reset,
}
