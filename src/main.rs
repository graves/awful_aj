//! # Awful Jade CLI Application
//!
//! Command-line interface for **Awful Jade** (“aj”), a local-first assistant that can
//! ask/answer questions, run interactive chats, and persist semantic memory via an
//! HNSW-based vector store. This binary wires together configuration, prompt templates,
//! session state, and the vector store brain, and provides three subcommands:
//!
//! - **`init`**: bootstrap default config and templates in the per-user config directory.
//! - **`ask`**: send a single question using a chosen template (optionally with session memory).
//! - **`interactive`**: open a REPL-like session that remembers context using embeddings.
//!
//! ## Model bootstrap
//!
//! On first run, the binary ensures the local sentence-embedding model
//! `all-mini-lm-l12-v2` exists under the per-user config directory
//! (see [`config_dir`]). If it is missing, the library’s
//! `awful_aj::ensure_all_mini()` will **download and unzip** the model into the
//! correct location. Subsequent runs reuse the on-disk model.
//!
//! ## Configuration & templates
//!
//! - Config lives at: `<config_dir>/config.yaml` (OS-specific; see [`config_dir`]).
//! - Templates live under: `<config_dir>/templates/`.
//!
//! The `init` flow creates reasonable defaults for both.
//!
//! ## Sessions & memory
//!
//! If a configuration contains a `session_name`, Awful Jade:
//!   1. Derives a stable SHA-256 digest from the session name,
//!   2. Loads (or creates) a per-session vector store YAML file
//!      `"<digest>_vector_store.yaml"` in the config directory, and
//!   3. Uses that store to retrieve “memories” (HNSW nearest-neighbors from
//!      `all-mini-lm-l12-v2` embeddings) that are eligible to be included
//!      with the current prompt.
//!
//! The proportion of the LLM context window dedicated to memory is controlled by a
//! fixed ratio (`max_brain_token_percentage = 0.25`), translated to a token budget
//! via the `context_max_tokens` setting in config.
//!
//! ## Subcommands (high level)
//!
//! - **`aj init`**  
//!   Creates `<config_dir>/config.yaml`, a default template (`default.yaml`),
//!   and a ready-to-edit example template (`simple_question.yaml`).
//!
//! - **`aj ask [--template <name>] [--session <name>] "your question"`**  
//!   Loads the chosen (or default) template, optionally binds to a named session,
//!   injects relevant memory (if any), and prints the assistant’s reply.
//!
//! - **`aj interactive [--template <name>] [--session <name>]`**  
//!   Starts an interactive loop that updates the session memory and renders replies
//!   until you exit.
//!
//! ## Testing path override
//!
//! When `IN_TEST_ENVIRONMENT` is set in the environment, configuration is loaded
//! from the current working directory (`./config.yaml`) instead of the per-user
//! config path. See [`determine_config_path`].

extern crate diesel;

use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestUserMessage};
use awful_aj::brain::Brain;
use awful_aj::vector_store::VectorStore;
use awful_aj::{api, commands, config, template};
use clap::Parser;
use directories::ProjectDirs;
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use std::{env, error::Error, fs, path::PathBuf, vec};
use tracing::{debug, info};

// A static OnceCell to hold the tracing subscriber, ensuring it is only initialized once.
static TRACING: OnceCell<()> = OnceCell::new();

/// Program entrypoint.
///
/// Initializes tracing, creates a Tokio runtime, and runs the async [`run`] function.
/// Any error returned by [`run`] is surfaced here.
///
/// # Returns
/// A standard `Result<(), Box<dyn Error>>`.
///
/// # Panics
/// Panics if a Tokio runtime cannot be created, or if blocking on the runtime fails.
///
/// # Examples
/// ```no_run
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Delegates to async runtime
///     // (this function is provided by the binary and normally not called directly)
///     Ok(())
/// }
/// ```
fn main() -> Result<(), Box<dyn Error>> {
    initialize_tracing();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run()).unwrap();
    Ok(())
}

/// Initialize global tracing.
///
/// Sets up the default tracing subscriber once per process. Safe to call repeatedly.
///
/// # Notes
/// Uses `tracing_subscriber::fmt::init()` with default formatting.
fn initialize_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt::init();
    });
}

/// Core async application logic.
///
/// - Parses CLI arguments via [`commands::Cli`] and dispatches to the selected subcommand.
/// - Ensures the sentence-embedding model is present by calling
///   `awful_aj::ensure_all_mini()` (downloads and unzips on first use).
/// - Loads configuration and templates from the per-user config directory unless
///   `IN_TEST_ENVIRONMENT` is set (see [`determine_config_path`]).
/// - For `ask`/`interactive`, loads or creates a per-session [`VectorStore`] (if a
///   session is active) and routes to the corresponding handler.
///
/// # Errors
/// Returns any I/O, (de)serialization, or API errors encountered during the run.
///
/// # Examples
/// ```no_run
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// // Normally called from main() via a Tokio runtime
/// // run().await?;
/// # Ok(()) }
/// ```
async fn run() -> Result<(), Box<dyn Error>> {
    let cli = commands::Cli::parse();

    match cli.command {
        commands::Commands::Ask {
            question,
            template,
            session,
            one_shot,
        } => {
            debug!("Entering ask mode");

            let config_path = determine_config_path()?;

            let config_str = config_path.to_str().ok_or_else(|| {
                format!("Invalid UTF-8 in config path: {}", config_path.display())
            })?;

            let mut jade_config = config::load_config(config_str).map_err(|e| {
                format!("Failed to load config at {}: {}", config_path.display(), e)
            })?;

            // If --one-shot flag is set, clear the session name from config
            if one_shot {
                jade_config.session_name = None;
                debug!("One-shot mode enabled - sessions disabled");
            }

            // Ensure conversation exists if session is provided via CLI or config
            if let Some(session_name) = session {
                jade_config
                    .ensure_conversation_and_config(&session_name)
                    .await?;
            } else if let Some(ref session_name) = jade_config.session_name.clone() {
                jade_config
                    .ensure_conversation_and_config(session_name)
                    .await?;
            }

            handle_ask_command(jade_config, question, template).await?;
        }
        commands::Commands::Interactive {
            template,
            session,
        } => {
            debug!("Entering interactive mode");

            let config_path = determine_config_path()?;
            let mut jade_config = config::load_config(config_path.to_str().unwrap())?;

            // Ensure conversation exists if session is provided via CLI or config
            if let Some(session_name) = session {
                jade_config
                    .ensure_conversation_and_config(&session_name)
                    .await?;
            } else if let Some(ref session_name) = jade_config.session_name.clone() {
                jade_config
                    .ensure_conversation_and_config(session_name)
                    .await?;
            }

            handle_interactive_command(jade_config, template).await?;
        }
        commands::Commands::Init { overwrite } => {
            debug!("Initializing configuration");
            init(overwrite)?;
        }
        commands::Commands::Reset => {
            debug!("Resetting database");
            let config_path = determine_config_path()?;
            let config_str = config_path.to_str().ok_or_else(|| {
                format!("Invalid UTF-8 in config path: {}", config_path.display())
            })?;
            let jade_config = config::load_config(config_str).map_err(|e| {
                format!("Failed to load config at {}: {}", config_path.display(), e)
            })?;
            reset(&jade_config)?;
        }
    }

    Ok(())
}

/// Handle the `ask` subcommand.
///
/// Loads the selected (or default) chat template and (optionally) a question.
/// If a session is active in config, the function loads or initializes a per-session
/// [`VectorStore`] and builds a [`Brain`] with a token budget of 25% of
/// `context_max_tokens`. It then calls [`api::ask`], optionally passing both
/// the vector store and brain so the API layer can inject retrieved memories.
///
/// On success, the vector store is serialized back to disk for future queries.
///
/// # Parameters
/// - `jade_config`: Loaded [`config::AwfulJadeConfig`].
/// - `question`: Optional question text. If `None`, defaults to
///   `"What is the meaning of life?"`.
/// - `template_name`: Optional template name. If `None`, defaults to `"simple_question"`.
///
/// # Errors
/// - Returns I/O errors when loading/saving files,
/// - YAML/JSON errors for (de)serialization,
/// - and API/template loading errors bubbled up from the `awful_aj` crate.
///
/// # Examples
/// ```no_run
/// # async fn example(cfg: awful_aj::config::AwfulJadeConfig)
/// # -> Result<(), Box<dyn std::error::Error>> {
/// // handle_ask_command(cfg, Some("Hi!".into()), Some("default".into())).await?;
/// # Ok(()) }
/// ```
async fn handle_ask_command(
    jade_config: config::AwfulJadeConfig,
    question: Option<String>,
    template_name: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let template_name = template_name.unwrap_or_else(|| "simple_question".to_string());
    let template = template::load_template(&template_name).await?;
    let question = question.unwrap_or_else(|| "What is the meaning of life?".to_string());

    if let Some(the_session_name) = jade_config.session_name.clone() {
        let digest = sha256::digest(&the_session_name);
        let vector_store_name = format!("{}_vector_store.yaml", digest);
        let vector_store_path = config_dir()?.join(vector_store_name);
        let vector_store_string = fs::read_to_string(&vector_store_path);

        let mut vector_store: VectorStore = if vector_store_string.is_ok() {
            serde_yaml::from_str(&vector_store_string.unwrap())?
        } else {
            VectorStore::new(384, jade_config.session_name.clone().unwrap())?
        };

        let max_brain_token_percentage = 0.25;
        let max_brain_tokens =
            (max_brain_token_percentage * jade_config.context_max_tokens as f32) as u16;

        let mut brain = Brain::new(max_brain_tokens, &template);

        api::ask(
            &jade_config,
            question,
            &template,
            Some(&mut vector_store),
            Some(&mut brain),
        )
        .await?;

        let _res = vector_store.serialize(
            &vector_store_path,
            jade_config.session_name.clone().unwrap(),
        );
    } else {
        api::ask(&jade_config, question, &template, None, None).await?;
    }

    Ok(())
}

/// Handle the `interactive` subcommand.
///
/// Opens an interactive loop backed by a per-session [`VectorStore`] and a
/// [`Brain`] instantiated with a 25% token budget against `context_max_tokens`.
/// The function delegates the loop mechanics to [`api::interactive_mode`].
///
/// # Parameters
/// - `jade_config`: Loaded [`config::AwfulJadeConfig`].
/// - `template_name`: Optional template name. If `None`, defaults to `"simple_question"`.
///
/// # Errors
/// Propagates template loading, I/O, (de)serialization, and API errors.
///
/// # Examples
/// ```no_run
/// # async fn example(cfg: awful_aj::config::AwfulJadeConfig)
/// # -> Result<(), Box<dyn std::error::Error>> {
/// // handle_interactive_command(cfg, Some("default".into())).await?;
/// # Ok(()) }
/// ```
async fn handle_interactive_command(
    jade_config: config::AwfulJadeConfig,
    template_name: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let template_name = template_name.unwrap_or_else(|| "simple_question".to_string());
    let template = template::load_template(&template_name).await?;

    // Load or create session-scoped vector store
    let the_session_name = jade_config.session_name.clone().unwrap();
    let digest = sha256::digest(&the_session_name);
    let vector_store_name = format!("{}_vector_store.yaml", digest);
    let vector_store_path = config_dir()?.join(vector_store_name);
    let vector_store_string = fs::read_to_string(&vector_store_path);

    let vector_store: VectorStore = if vector_store_string.is_ok() {
        serde_yaml::from_str(&vector_store_string.unwrap())?
    } else {
        VectorStore::new(384, jade_config.session_name.clone().unwrap())?
    };

    // Brain token budget = 25% of configured context window
    let max_brain_token_percentage = 0.25;
    let max_brain_tokens =
        (max_brain_token_percentage * jade_config.context_max_tokens as f32) as u16;
    let brain = Brain::new(max_brain_tokens, &template);
    api::interactive_mode(&jade_config, vector_store, brain, &template).await
}

/// Compute the path of the active configuration file.
///
/// - In **test mode** (`IN_TEST_ENVIRONMENT` is set), this returns `./config.yaml`.
/// - Otherwise, it returns `<config_dir>/config.yaml`, where `config_dir`
///   is derived via [`directories::ProjectDirs`] with the tuple
///   `("com", "awful-sec", "aj")`.
///
/// # Returns
/// Absolute path to `config.yaml`.
///
/// # Errors
/// Returns an error if the current working directory cannot be read (test mode),
/// or if the per-user config directory cannot be determined.
///
/// # Examples
/// ```no_run
/// let path = determine_config_path()?;
/// ```
fn determine_config_path() -> Result<PathBuf, Box<dyn Error>> {
    if env::var("IN_TEST_ENVIRONMENT").is_ok() {
        Ok(env::current_dir()?.join("config.yaml")) // Test environment
    } else {
        Ok(config_dir()?.join("config.yaml")) // User's config directory
    }
}

use async_openai::types::ChatCompletionRequestSystemMessage;
use async_openai::types::ChatCompletionRequestSystemMessageContent;
use async_openai::types::ChatCompletionRequestUserMessageContent;

/// Initialize per-user configuration files and templates.
///
/// This creates the templates directory and writes both:
/// - `templates/simple_question.yaml` — an example prompt file with a system + user message.
/// - `templates/default.yaml` — a minimal default template with a system prompt.
/// - `config.yaml` — a baseline configuration file with local defaults.
///
/// The function is **idempotent** with respect to directories and will overwrite the
/// template files and config if they already exist.
///
/// # Returns
/// `Ok(())` on success.
///
/// # Errors
/// Returns any file I/O or serialization errors.
///
/// # Examples
/// ```no_run
/// init()?;
/// ```
fn init(overwrite: bool) -> Result<(), Box<dyn Error>> {
    let config_dir = config_dir()?;
    let path = config_dir.join("templates");
    info!("Creating template config directory: {}", path.display());
    fs::create_dir_all(path.clone())?;

    // Write example template (simple_question.yaml)
    let template_path = config_dir.join("templates/simple_question.yaml");
    
    if template_path.exists() && !overwrite {
        info!("Template file already exists (skipping): {}", template_path.display());
    } else {
        info!("Creating template file: {}", template_path.display());
    let user_message = ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
        content: ChatCompletionRequestUserMessageContent::Text(
            "How do I read a file in Rust?".to_string(),
        ),
        name: None,
    });

    // A didactic system message with sample Rust code; this is just an example template.
    let system_message_content = "Use `std::fs::File` and `std::io::Read` in Rust to read a file:
```rust
use std::fs::File;
use std::io::{self, Read};

fn main() -> io::Result<()> {
    let mut file = File::open(\"file.txt\")?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    println!(\"{}\", content);
    Ok(())
}
```"
    .to_string();

    let system_message = ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
        content: ChatCompletionRequestSystemMessageContent::Text(system_message_content),
        name: None,
    });

    // Also write a minimal default template
    let template = template::ChatTemplate {
        system_prompt: "You are Awful Jade, a helpful AI assistant programmed by Awful Security."
            .to_string(),
        messages: vec![user_message, system_message],
        response_format: None,
        pre_user_message_content: None,
        post_user_message_content: None,
    };
        let template_yaml = serde_yaml::to_string(&template)?;
        fs::write(template_path, template_yaml)?;
    }
    
    // Create the default template
    create_default_template(&path, overwrite)?;

    // Baseline config file with local defaults
    let config_path = config_dir.join("config.yaml");
    
    if config_path.exists() && !overwrite {
        info!("Config file already exists (skipping): {}", config_path.display());
    } else {
        info!("Creating config file: {}", config_path.display());
        // Use absolute path for database to avoid CWD issues
        let db_absolute_path = config_dir.join("aj.db");
        let config = config::AwfulJadeConfig {
            api_base: "http://localhost:5001/v1".to_string(),
            api_key: "CHANGEME".to_string(),
            model: "jade_qwen3_4b".to_string(),
            context_max_tokens: 8192,
            assistant_minimum_context_tokens: 2048,
            stop_words: vec!["\n<|im_start|>".to_string(), "<|im_end|>".to_string()],
            session_db_url: db_absolute_path.to_string_lossy().to_string(),
            session_name: None,
            should_stream: None,
        };
        let config_yaml = serde_yaml::to_string(&config)?;
        fs::write(config_path, config_yaml)?;
    }

    // Create SQLite database with schema
    let db_path = config_dir.join("aj.db");
    
    if db_path.exists() && !overwrite {
        info!("Database file already exists (skipping): {}", db_path.display());
    } else {
        info!("Creating database file: {}", db_path.display());
        create_database(&db_path)?;
    }

    Ok(())
}

/// Create and initialize the SQLite database with the required schema.
///
/// # Parameters
/// - `db_path`: Path where the database file should be created.
///
/// # Returns
/// `Ok(())` on success.
///
/// # Errors
/// Returns database errors if creation or schema execution fails.
fn create_database(db_path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open(db_path)?;
    
    // Execute the schema
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS awful_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            api_base TEXT NOT NULL,
            api_key TEXT NOT NULL,
            model TEXT NOT NULL,
            context_max_tokens INTEGER NOT NULL,
            assistant_minimum_context_tokens INTEGER NOT NULL,
            stop_words TEXT NOT NULL,
            conversation_id INTEGER,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id)
        );

        CREATE TABLE IF NOT EXISTS conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            session_name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            dynamic BOOLEAN NOT NULL DEFAULT true,
            conversation_id INTEGER,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id)
        );
        "#,
    )?;
    
    info!("Database initialized successfully");
    Ok(())
}

/// Reset the database to a pristine state.
///
/// This function reads the database path from the config (respecting the user's
/// configured path), deletes the database file, and recreates it with the original schema.
///
/// # Parameters
/// - `config`: Application configuration to determine which database to reset.
///
/// # Returns
/// `Ok(())` on success.
///
/// # Errors
/// Returns database errors if the reset operation fails.
///
/// # Examples
/// ```no_run
/// let cfg = load_config("config.yaml")?;
/// reset(&cfg)?;
/// ```
fn reset(config: &config::AwfulJadeConfig) -> Result<(), Box<dyn Error>> {
    let db_path = std::path::PathBuf::from(&config.session_db_url);
    
    if !db_path.exists() {
        info!("Database file does not exist at: {}", db_path.display());
        info!("Creating new database...");
        create_database(&db_path)?;
        return Ok(());
    }
    
    info!("Resetting database at: {}", db_path.display());
    
    // Close any existing connections by deleting and recreating the file
    // This ensures no stale connections interfere with the reset
    fs::remove_file(&db_path)?;
    info!("Deleted existing database file");
    
    // Create fresh database with schema
    create_database(&db_path)?;
    
    info!("Database reset successfully - all data cleared");
    Ok(())
}

/// Write the built-in default template (`templates/default.yaml`).
///
/// The template contains only a system prompt and an empty `messages` array.
/// This file provides a simple, predictable default for quick experiments.
///
/// # Parameters
/// - `templates_dir`: Directory where the file will be created.
///
/// # Returns
/// `Ok(())` on success.
///
/// # Errors
/// Returns I/O errors when creating or writing the file.
///
/// # Examples
/// ```no_run
/// let dir = std::path::Path::new("/some/config/templates");
/// create_default_template(dir)?;
/// ```
fn create_default_template(templates_dir: &std::path::Path, overwrite: bool) -> Result<(), Box<dyn Error>> {
    let default_template_path = templates_dir.join("default.yaml");
    
    if default_template_path.exists() && !overwrite {
        info!(
            "Default template file already exists (skipping): {}",
            default_template_path.display()
        );
    } else {
        info!(
            "Creating default template file: {}",
            default_template_path.display()
        );
        // Minimal template with only a system prompt
        let default_template_content = r#"
system_prompt: "Your name is Awful Jade, you are a helpful AI assistant programmed by Awful Security."
messages: []
"#;
        fs::write(default_template_path, default_template_content)?;
    }
    Ok(())
}

/// Resolve the per-user configuration directory.
///
/// Uses [`directories::ProjectDirs`] with the tuple `("com", "awful-sec", "aj")`
/// to compute an OS-appropriate configuration directory:
///
/// - **macOS**: `~/Library/Application Support/com.awful-sec.aj`
/// - **Linux**: `~/.config/aj`
/// - **Windows**: `%APPDATA%\awful-sec\aj`
///
/// This location is used for `config.yaml`, the `templates/` folder, the
/// per-session vector store YAMLs, and the downloaded `all-mini-lm-l12-v2` model.
///
/// # Returns
/// Absolute path to the config directory.
///
/// # Errors
/// Returns an error if the directory cannot be determined (rare; indicates a
/// nonstandard environment).
///
/// # Examples
/// ```no_run
/// let root = config_dir()?;
/// println!("config root: {}", root.display());
/// ```
pub fn config_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let proj_dirs = ProjectDirs::from("com", "awful-sec", "aj")
        .ok_or("Unable to determine config directory")?;
    let config_dir = proj_dirs.config_dir().to_path_buf();

    Ok(config_dir)
}
