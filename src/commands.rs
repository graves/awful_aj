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

/// Top-level CLI parser for the Awful Jade application.
///
/// This struct is the entry point for command-line argument parsing and is produced
/// by [`clap::Parser::parse`]. It contains exactly one selected subcommand in
/// [`Cli::command`].
///
/// # Examples
///
/// ```no_run
/// use awful_aj::commands::{Cli, Commands};
/// use clap::Parser;
///
/// let cli = Cli::parse();
/// match cli.command {
///     Commands::Ask { .. } => { /* handle ask */ },
///     Commands::Interactive { .. } => { /* handle interactive */ },
///     Commands::Init { .. } => { /* handle init */ },
///     Commands::Reset => { /* handle reset */ },
/// }
/// ```
///
/// # CLI Features
///
/// - Colorized help output (always enabled)
/// - Version propagation to all subcommands
/// - Short and long argument forms for all options
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
    /// The subcommand to execute.
    ///
    /// One of: `ask`, `interactive`, `init`, or `reset`.
    #[command(subcommand)]
    pub command: Commands,
}

/// All supported subcommands for the Awful Jade CLI.
///
/// Each variant represents a distinct mode of operation. See the individual
/// variant documentation for detailed information about each subcommand's
/// parameters and behavior.
///
/// # Subcommands
///
/// - [`Ask`](Commands::Ask): Single-shot question answering with optional RAG
/// - [`Interactive`](Commands::Interactive): REPL-style chat session
/// - [`Init`](Commands::Init): Configuration and template initialization
/// - [`Reset`](Commands::Reset): Database cleanup and schema recreation
///
/// # Examples
///
/// ```bash
/// # Ask a single question
/// aj ask "What is HNSW?"
///
/// # Start interactive session with template
/// aj interactive -t simple_question
///
/// # Initialize configuration
/// aj init
///
/// # Reset database
/// aj reset
/// ```
#[derive(Subcommand, Debug)]
#[command(about, long_about = None, color = clap::ColorChoice::Always)]
pub enum Commands {
    /// Ask a single question and print the assistant's response.
    ///
    /// This mode sends a single query to the LLM and prints the response.
    /// If no question is provided, the application uses a default prompt.
    ///
    /// # Features
    ///
    /// - Optional session persistence for context continuity
    /// - RAG support for document-based question answering
    /// - Pretty printing with markdown rendering and syntax highlighting
    /// - One-shot mode to ignore configured sessions
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Basic question
    /// aj ask "Explain HNSW indexing"
    ///
    /// # With template and session
    /// aj ask -t technical -s project-x "How does vector search work?"
    ///
    /// # With RAG documents
    /// aj ask -r "doc1.txt,doc2.txt" -k 5 "Summarize the key points"
    ///
    /// # One-shot mode (ignore session)
    /// aj ask -o "What is the capital of France?"
    ///
    /// # Pretty output
    /// aj ask -p "Write a Python hello world"
    /// ```
    ///
    /// Aliases: `a`
    #[clap(name = "ask", alias = "a")]
    Ask {
        /// The question to ask the assistant.
        ///
        /// When omitted, the application uses a default question specified
        /// in the configuration or template.
        question: Option<String>,

        /// Name of the chat template to load (e.g., `simple_question`).
        ///
        /// Templates are YAML files that define system prompts, message seeds,
        /// and response formatting. They live under the app's config directory:
        ///
        /// - **macOS**: `~/Library/Application Support/com.awful-sec.aj/templates/`
        /// - **Linux**: `~/.config/aj/templates/`
        /// - **Windows**: `%APPDATA%\\com.awful-sec\\aj\\templates\\`
        ///
        /// Use `aj init` to create default templates.
        #[arg(name = "template", short = 't')]
        template: Option<String>,

        /// Session name for conversation persistence.
        ///
        /// When set, messages are stored in the SQLite database under this
        /// conversation name. Using a session enables:
        ///
        /// - Context continuity across multiple `ask` calls
        /// - Retrieval-augmented context from prior turns via vector search
        /// - Conversation history tracking
        ///
        /// If not specified, uses the session name from `config.yaml` (if configured).
        #[arg(name = "session", short = 's')]
        session: Option<String>,

        /// Force one-shot mode, ignoring any session configured in config.yaml.
        ///
        /// When this flag is set, the prompt is treated as standalone with no
        /// memory or session tracking, even if `session_name` is configured in
        /// the config file. This is useful for quick, isolated queries.
        ///
        /// **Note**: Overrides both `-s/--session` and the config file setting.
        #[arg(short = 'o', long)]
        one_shot: bool,

        /// Comma-separated list of plain text files for RAG (Retrieval-Augmented Generation) context.
        ///
        /// When provided, these documents are:
        ///
        /// 1. Split into overlapping chunks (512 tokens with 128 token overlap)
        /// 2. Embedded using the sentence transformer model (`all-MiniLM-L6-v2`)
        /// 3. Indexed in a temporary HNSW vector store
        /// 4. Retrieved based on semantic similarity to the question
        /// 5. Injected into the prompt preamble as context
        ///
        /// This enables the model to answer questions based on document content
        /// even if the information wasn't in its training data.
        ///
        /// # Example
        ///
        /// ```bash
        /// aj ask -r "manual.txt,faq.txt" -k 5 "How do I configure the API?"
        /// ```
        #[arg(short = 'r', long)]
        rag: Option<String>,

        /// Maximum number of RAG chunks to inject into the context.
        ///
        /// Controls how many of the most relevant document chunks are retrieved
        /// from the vector store and added to the prompt. Higher values provide
        /// more context but consume more tokens.
        ///
        /// **Default**: 3 chunks
        #[arg(short = 'k', long, default_value = "3")]
        rag_top_k: usize,

        /// Enable pretty-printing with markdown rendering and syntax highlighting.
        ///
        /// When enabled, the assistant's response is formatted with:
        ///
        /// - **Markdown rendering**: Headers, bold, italic, lists
        /// - **Syntax highlighting**: Language-aware code block coloring
        /// - **Stream-then-replace**: Shows raw streaming output, then replaces
        ///   with formatted version
        ///
        /// Uses the `base16-ocean.dark` theme from Syntect.
        #[arg(short = 'p', long)]
        pretty: bool,
    },

    /// Start an interactive REPL-style conversation.
    ///
    /// Enters a read-eval-print loop where you can have a multi-turn conversation
    /// with the assistant. The session continues until you type `exit`, `quit`, or press Ctrl+C.
    ///
    /// # Features
    ///
    /// - **Multi-turn conversation**: Maintain context across multiple exchanges
    /// - **Session persistence**: All messages saved to SQLite database
    /// - **Streaming output**: See responses token-by-token (when enabled in config)
    /// - **RAG support**: Load documents once, query across all turns
    /// - **Memory retrieval**: Automatically fetch relevant past messages via vector search
    /// - **Pretty printing**: Optional markdown rendering and syntax highlighting
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Basic interactive mode
    /// aj interactive
    ///
    /// # With specific session and template
    /// aj interactive -s my-project -t technical
    ///
    /// # With RAG documents loaded at startup
    /// aj interactive -r "docs/*.txt" -k 5
    ///
    /// # With pretty output
    /// aj interactive -p
    /// ```
    ///
    /// # Workflow
    ///
    /// 1. User enters prompt at `You: ` prompt
    /// 2. System retrieves relevant memories from vector store
    /// 3. Brain assembles context (preamble + RAG + memories + current prompt)
    /// 4. LLM generates response (streamed or buffered)
    /// 5. Response saved to session database
    /// 6. Loop continues until exit
    ///
    /// Aliases: `i`
    #[clap(name = "interactive", alias = "i")]
    Interactive {
        /// Name of the chat template to load (e.g., `simple_question`).
        ///
        /// Templates define system prompts and message structure. See the `Ask`
        /// command documentation for template directory locations.
        #[arg(name = "template", short = 't')]
        template: Option<String>,

        /// Session name for conversation persistence.
        ///
        /// All messages in the interactive session are saved under this name.
        /// If not specified, uses the session name from `config.yaml`.
        ///
        /// **Tip**: Use descriptive session names like `project-refactor` or
        /// `debugging-auth` to organize conversations by topic.
        #[arg(name = "session", short = 's')]
        session: Option<String>,

        /// Comma-separated list of plain text files for RAG context.
        ///
        /// Documents are loaded once at startup and remain available for all
        /// queries in the interactive session. The vector store is built during
        /// initialization and queried on each user prompt.
        ///
        /// This is more efficient than using RAG in `ask` mode repeatedly, as
        /// the embeddings are computed only once.
        ///
        /// # Example
        ///
        /// ```bash
        /// aj interactive -r "README.md,CONTRIBUTING.md,docs/api.md" -k 5
        /// ```
        ///
        /// Now you can ask questions like "How do I contribute?" and the assistant
        /// will have access to all three documents.
        #[arg(short = 'r', long)]
        rag: Option<String>,

        /// Maximum number of RAG chunks to inject per query.
        ///
        /// Each time you submit a prompt in interactive mode, the system retrieves
        /// up to this many chunks from the RAG document store based on semantic
        /// similarity to your query.
        ///
        /// **Default**: 3 chunks
        #[arg(short = 'k', long, default_value = "3")]
        rag_top_k: usize,

        /// Enable pretty-printing with markdown rendering and syntax highlighting.
        ///
        /// When enabled, responses are formatted with markdown styling and code
        /// blocks are syntax-highlighted. In streaming mode, raw output is shown
        /// first, then replaced with the formatted version.
        #[arg(short = 'p', long)]
        pretty: bool,
    },

    /// Initialize configuration and default templates in the platform config directory.
    ///
    /// Creates the necessary files and directories for Awful Jade to function:
    ///
    /// 1. **Configuration directory** (platform-specific):
    ///    - macOS: `~/Library/Application Support/com.awful-sec.aj/`
    ///    - Linux: `~/.config/aj/`
    ///    - Windows: `%APPDATA%\\com.awful-sec\\aj\\`
    ///
    /// 2. **config.yaml** with default settings:
    ///    - OpenAI-compatible API endpoint
    ///    - Model name and token limits
    ///    - Streaming preferences
    ///    - Stop words for chat templates
    ///    - SQLite database path
    ///
    /// 3. **Template directory** with default templates:
    ///    - `simple_question.yaml`: Basic Q&A template
    ///    - Additional starter templates
    ///
    /// 4. **SQLite database** with schema initialization
    ///
    /// # Examples
    ///
    /// ```bash
    /// # First-time setup
    /// aj init
    ///
    /// # Force overwrite existing files (useful for resetting config)
    /// aj init -f
    /// ```
    ///
    /// # Safety
    ///
    /// By default, `init` will **not** overwrite existing files. Use the `-f/--overwrite`
    /// flag to force overwriting.
    Init {
        /// Overwrite existing files (config, templates, database).
        ///
        /// By default, `init` preserves existing files to avoid data loss. When this
        /// flag is set, all files are recreated from defaults.
        ///
        /// **Warning**: This will erase any customizations you've made to templates
        /// or configuration files.
        #[arg(short = 'f', long)]
        overwrite: bool,
    },

    /// Reset the database to a pristine state.
    ///
    /// This command performs a hard reset of the SQLite database:
    ///
    /// 1. **Drops all tables**: Sessions, messages, configurations
    /// 2. **Recreates schema**: Runs migrations to rebuild tables
    /// 3. **Clears vector stores**: Removes all YAML index files
    ///
    /// # Use Cases
    ///
    /// - Start fresh after experimenting with sessions
    /// - Recover from database corruption
    /// - Clear all conversation history
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Reset database
    /// aj reset
    /// ```
    ///
    /// # Safety
    ///
    /// **Warning**: This is a destructive operation. All conversation history,
    /// sessions, and vector store indices will be permanently deleted. There is
    /// no confirmation prompt, so use with caution.
    ///
    /// # Technical Details
    ///
    /// The reset process:
    ///
    /// - Locates the database path from `config.yaml` (`session_db_url`)
    /// - Drops tables: `sessions`, `messages`
    /// - Recreates tables using Diesel migrations
    /// - Searches for `*_vector_store.yaml` and `*_hnsw_index.bin` files and removes them
    ///
    /// Aliases: `r`
    #[clap(name = "reset", alias = "r")]
    Reset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_verify_basic_structure() {
        // Ensure the Cli struct can be instantiated
        use clap::CommandFactory;
        let _cmd = Cli::command();
    }

    #[test]
    fn test_ask_command_basic() {
        let args = vec!["aj", "ask", "What is Rust?"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { question, .. } => {
                    assert_eq!(question, Some("What is Rust?".to_string()));
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_ask_command_with_template() {
        let args = vec!["aj", "ask", "-t", "simple_question", "What is HNSW?"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask {
                    template, question, ..
                } => {
                    assert_eq!(template, Some("simple_question".to_string()));
                    assert_eq!(question, Some("What is HNSW?".to_string()));
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_ask_command_with_session() {
        let args = vec!["aj", "ask", "-s", "test-session", "Test question"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { session, .. } => {
                    assert_eq!(session, Some("test-session".to_string()));
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_ask_command_one_shot() {
        let args = vec!["aj", "ask", "-o", "Quick question"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { one_shot, .. } => {
                    assert!(one_shot);
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_ask_command_with_rag() {
        let args = vec![
            "aj",
            "ask",
            "-r",
            "doc1.txt,doc2.txt",
            "-k",
            "5",
            "Question",
        ];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { rag, rag_top_k, .. } => {
                    assert_eq!(rag, Some("doc1.txt,doc2.txt".to_string()));
                    assert_eq!(rag_top_k, 5);
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_ask_command_with_pretty() {
        let args = vec!["aj", "ask", "-p", "Test"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { pretty, .. } => {
                    assert!(pretty);
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_ask_command_alias() {
        let args = vec!["aj", "a", "Question"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { .. } => {
                    // Success - alias works
                }
                _ => panic!("Expected Ask command via alias"),
            }
        }
    }

    #[test]
    fn test_interactive_command_basic() {
        let args = vec!["aj", "interactive"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Interactive { .. } => {
                    // Success
                }
                _ => panic!("Expected Interactive command"),
            }
        }
    }

    #[test]
    fn test_interactive_command_with_options() {
        let args = vec!["aj", "interactive", "-t", "template", "-s", "session", "-p"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Interactive {
                    template,
                    session,
                    pretty,
                    ..
                } => {
                    assert_eq!(template, Some("template".to_string()));
                    assert_eq!(session, Some("session".to_string()));
                    assert!(pretty);
                }
                _ => panic!("Expected Interactive command"),
            }
        }
    }

    #[test]
    fn test_interactive_command_with_rag() {
        let args = vec!["aj", "interactive", "-r", "docs.txt", "-k", "10"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Interactive { rag, rag_top_k, .. } => {
                    assert_eq!(rag, Some("docs.txt".to_string()));
                    assert_eq!(rag_top_k, 10);
                }
                _ => panic!("Expected Interactive command"),
            }
        }
    }

    #[test]
    fn test_interactive_command_alias() {
        let args = vec!["aj", "i"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Interactive { .. } => {
                    // Success - alias works
                }
                _ => panic!("Expected Interactive command via alias"),
            }
        }
    }

    #[test]
    fn test_init_command_basic() {
        let args = vec!["aj", "init"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Init { overwrite } => {
                    assert!(!overwrite);
                }
                _ => panic!("Expected Init command"),
            }
        }
    }

    #[test]
    fn test_init_command_with_overwrite() {
        let args = vec!["aj", "init", "-f"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Init { overwrite } => {
                    assert!(overwrite);
                }
                _ => panic!("Expected Init command"),
            }
        }
    }

    #[test]
    fn test_reset_command_basic() {
        let args = vec!["aj", "reset"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Reset => {
                    // Success
                }
                _ => panic!("Expected Reset command"),
            }
        }
    }

    #[test]
    fn test_reset_command_alias() {
        let args = vec!["aj", "r"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Reset => {
                    // Success - alias works
                }
                _ => panic!("Expected Reset command via alias"),
            }
        }
    }

    #[test]
    fn test_rag_default_top_k() {
        let args = vec!["aj", "ask", "-r", "docs.txt", "Question"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Ask { rag_top_k, .. } => {
                    assert_eq!(rag_top_k, 3); // Default value
                }
                _ => panic!("Expected Ask command"),
            }
        }
    }

    #[test]
    fn test_invalid_command() {
        let args = vec!["aj", "nonexistent"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_err());
    }
}
