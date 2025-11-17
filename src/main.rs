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
//! `sentence-transformers/all-MiniLM-L6-v2` exists under the per-user config directory
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
//!      `all-MiniLM-L6-v2` embeddings with 384-dim vectors) that are eligible to be included
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
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, error::Error, fs, path::PathBuf, vec};
use tracing::{debug, info};

use serde::{Deserialize, Serialize};
use std::io::Read;

// ---- RAG cache types & helpers (bincode-backed) ----

#[derive(Debug, Serialize, Deserialize)]
struct CachedChunk {
    text: String,
    vector: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RagCacheFile {
    version: u8,
    model_id: String,
    chunk_size: usize,
    overlap: usize,
    file_hash: String,
    created_unix: i64,
    chunks: Vec<CachedChunk>,
}

fn rag_cache_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let root = config_dir()?;
    let dir = root.join("rag_cache");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn hash_bytes_sha(path: &str) -> Result<String, Box<dyn Error>> {
    let mut f = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn cache_key_path(
    file_hash: &str,
    model_id: &str,
    chunk_size: usize,
    overlap: usize,
) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let safe_model = model_id.replace('/', "_");
    Ok(rag_cache_dir()?.join(format!(
        "{}__{}__cs{}__ov{}.bin",
        file_hash, safe_model, chunk_size, overlap
    )))
}

fn try_load_cache(
    file_hash: &str,
    model_id: &str,
    chunk_size: usize,
    overlap: usize,
) -> Result<Option<RagCacheFile>, Box<dyn Error>> {
    let path = cache_key_path(file_hash, model_id, chunk_size, overlap)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    // bincode v2:
    let (cache, _len): (RagCacheFile, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
    if cache.version != 1
        || cache.model_id != model_id
        || cache.chunk_size != chunk_size
        || cache.overlap != overlap
        || cache.file_hash != file_hash
    {
        return Ok(None);
    }
    Ok(Some(cache))
}

fn save_cache(cache: &RagCacheFile) -> Result<(), Box<dyn Error>> {
    let path = cache_key_path(
        &cache.file_hash,
        &cache.model_id,
        cache.chunk_size,
        cache.overlap,
    )?;
    // bincode v2:
    let bytes = bincode::serde::encode_to_vec(cache, bincode::config::standard())?;
    fs::write(path, bytes)?;
    Ok(())
}

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
            rag,
            rag_top_k,
            pretty,
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

            handle_ask_command(jade_config, question, template, rag, rag_top_k, pretty).await?;
        }
        commands::Commands::Interactive {
            template,
            session,
            rag,
            rag_top_k,
            pretty,
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

            handle_interactive_command(jade_config, template, rag, rag_top_k, pretty).await?;
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
    rag: Option<String>,
    rag_top_k: usize,
    pretty: bool,
) -> Result<(), Box<dyn Error>> {
    let template_name = template_name.unwrap_or_else(|| "simple_question".to_string());
    let template = template::load_template(&template_name).await?;
    let question = question.unwrap_or_else(|| "What is the meaning of life?".to_string());

    // Process RAG documents if provided
    let rag_context = if let Some(rag_files) = rag {
        use crossterm::{
            ExecutableCommand,
            style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
        };
        use std::io::stdout;

        let mut stdout = stdout();
        stdout.execute(SetForegroundColor(Color::Cyan))?;
        stdout.execute(SetAttribute(Attribute::Bold))?;
        stdout.execute(Print("📚 Processing RAG documents..."))?;
        stdout.execute(SetAttribute(Attribute::Reset))?;
        stdout.execute(SetForegroundColor(Color::Reset))?;
        stdout.execute(Print("\n"))?;

        let context = process_rag_documents(&rag_files, &question, rag_top_k)?;

        if !context.is_empty() {
            stdout.execute(SetForegroundColor(Color::Cyan))?;
            stdout.execute(SetAttribute(Attribute::Bold))?;
            stdout.execute(Print("✓ RAG context injected into conversation\n"))?;
            stdout.execute(SetAttribute(Attribute::Reset))?;
            stdout.execute(SetForegroundColor(Color::Reset))?;
        }

        Some(context)
    } else {
        None
    };

    if let Some(the_session_name) = jade_config.session_name.clone() {
        let digest = sha256::digest(&the_session_name);
        let vector_store_name = format!("{}_vector_store.yaml", digest);
        let vector_store_path = config_dir()?.join(vector_store_name);
        let vector_store_string = fs::read_to_string(&vector_store_path);

        let mut vector_store: VectorStore = if let Ok(yaml_content) = vector_store_string {
            // Try to deserialize, but if it fails (e.g., missing binary index file), create new
            match serde_yaml::from_str(&yaml_content) {
                Ok(store) => store,
                Err(e) => {
                    debug!("Failed to load vector store, creating new one: {}", e);
                    VectorStore::new(384, jade_config.session_name.clone().unwrap())?
                }
            }
        } else {
            VectorStore::new(384, jade_config.session_name.clone().unwrap())?
        };

        let max_brain_token_percentage = 0.25;
        let max_brain_tokens =
            (max_brain_token_percentage * jade_config.context_max_tokens as f32) as u16;

        let mut brain = Brain::new(max_brain_tokens, &template);

        // Set RAG context if available
        brain.rag_context = rag_context;

        let response = api::ask(
            &jade_config,
            question,
            &template,
            Some(&mut vector_store),
            Some(&mut brain),
            pretty,
        )
        .await?;

        // Print response if not streaming (streaming prints inline)
        if jade_config.should_stream != Some(true) {
            if pretty {
                // Use pretty printer for markdown formatting and syntax highlighting
                awful_aj::pretty::print_pretty(&response)?;
            } else {
                // Plain output
                use crossterm::{
                    ExecutableCommand,
                    style::{Attribute, Color, SetAttribute, SetForegroundColor},
                };
                use std::io::stdout;
                let mut out = stdout();
                out.execute(SetForegroundColor(Color::Yellow))?;
                out.execute(SetAttribute(Attribute::Bold))?;
                println!("{}", response);
                out.execute(SetAttribute(Attribute::Reset))?;
                out.execute(SetForegroundColor(Color::Reset))?;
            }
        }

        // Persist vector store to YAML (avoid serde::Serialize::serialize name clash)
        if let Ok(file) = fs::File::create(&vector_store_path) {
            if let Err(e) = serde_yaml::to_writer(file, &vector_store) {
                debug!(
                    "Failed to persist vector store to {}: {}",
                    vector_store_path.display(),
                    e
                );
            }
        }
    } else {
        let mut brain_opt = None;
        if rag_context.is_some() {
            let mut brain = Brain::new(2048, &template);
            brain.rag_context = rag_context;
            brain_opt = Some(brain);
        }

        let response = if let Some(mut brain) = brain_opt {
            api::ask(
                &jade_config,
                question,
                &template,
                None,
                Some(&mut brain),
                pretty,
            )
            .await?
        } else {
            api::ask(&jade_config, question, &template, None, None, pretty).await?
        };

        // Print response if not streaming (streaming prints inline)
        if jade_config.should_stream != Some(true) {
            if pretty {
                // Use pretty printer for markdown formatting and syntax highlighting
                awful_aj::pretty::print_pretty(&response)?;
            } else {
                // Plain output
                use crossterm::{
                    ExecutableCommand,
                    style::{Attribute, Color, SetAttribute, SetForegroundColor},
                };
                use std::io::stdout;
                let mut out = stdout();
                out.execute(SetForegroundColor(Color::Yellow))?;
                out.execute(SetAttribute(Attribute::Bold))?;
                println!("{}", response);
                out.execute(SetAttribute(Attribute::Reset))?;
                out.execute(SetForegroundColor(Color::Reset))?;
            }
        }
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
    rag: Option<String>,
    rag_top_k: usize,
    pretty: bool,
) -> Result<(), Box<dyn Error>> {
    let template_name = template_name.unwrap_or_else(|| "simple_question".to_string());
    let template = template::load_template(&template_name).await?;

    // Process RAG documents if provided
    let rag_context = if let Some(rag_files) = rag {
        use crossterm::{
            ExecutableCommand,
            style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
        };
        use std::io::stdout;

        let mut stdout = stdout();
        stdout.execute(SetForegroundColor(Color::Cyan))?;
        stdout.execute(SetAttribute(Attribute::Bold))?;
        stdout.execute(Print("📚 Processing RAG documents..."))?;
        stdout.execute(SetAttribute(Attribute::Reset))?;
        stdout.execute(SetForegroundColor(Color::Reset))?;
        stdout.execute(Print("\n"))?;

        // Use empty query for initial processing - context will be used for all queries in session
        let context = process_rag_documents(&rag_files, "", rag_top_k)?;

        if !context.is_empty() {
            stdout.execute(SetForegroundColor(Color::Cyan))?;
            stdout.execute(SetAttribute(Attribute::Bold))?;
            stdout.execute(Print(
                "✓ RAG context loaded and will be available throughout the session\n",
            ))?;
            stdout.execute(SetAttribute(Attribute::Reset))?;
            stdout.execute(SetForegroundColor(Color::Reset))?;
        }

        Some(context)
    } else {
        None
    };

    // Load or create session-scoped vector store
    let the_session_name = jade_config.session_name.clone().unwrap();
    let digest = sha256::digest(&the_session_name);
    let vector_store_name = format!("{}_vector_store.yaml", digest);
    let vector_store_path = config_dir()?.join(vector_store_name);
    let vector_store_string = fs::read_to_string(&vector_store_path);

    let vector_store: VectorStore = if let Ok(yaml_content) = vector_store_string {
        // Try to deserialize, but if it fails (e.g., missing binary index file), create new
        match serde_yaml::from_str(&yaml_content) {
            Ok(store) => store,
            Err(e) => {
                debug!("Failed to load vector store, creating new one: {}", e);
                VectorStore::new(384, jade_config.session_name.clone().unwrap())?
            }
        }
    } else {
        VectorStore::new(384, jade_config.session_name.clone().unwrap())?
    };

    // Brain token budget = 25% of configured context window
    let max_brain_token_percentage = 0.25;
    let max_brain_tokens =
        (max_brain_token_percentage * jade_config.context_max_tokens as f32) as u16;
    let mut brain = Brain::new(max_brain_tokens, &template);

    // Set RAG context if available
    brain.rag_context = rag_context;

    api::interactive_mode(&jade_config, vector_store, brain, &template, pretty).await
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
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.enable_steady_tick(Duration::from_millis(80));

    let config_dir = config_dir()?;
    let path = config_dir.join("templates");

    pb.set_message("Creating template directory...");
    info!("Creating template config directory: {}", path.display());
    fs::create_dir_all(path.clone())?;

    // Write example template (simple_question.yaml)
    let template_path = config_dir.join("templates/simple_question.yaml");

    if template_path.exists() && !overwrite {
        pb.set_message("Template file already exists (skipping)...");
        info!(
            "Template file already exists (skipping): {}",
            template_path.display()
        );
    } else {
        pb.set_message("Writing simple_question template...");
        info!("Creating template file: {}", template_path.display());
        let user_message = ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(
                "How do I read a file in Rust?".to_string(),
            ),
            name: None,
        });

        // A didactic system message with sample Rust code; this is just an example template.
        let system_message_content =
            "Use `std::fs::File` and `std::io::Read` in Rust to read a file:
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

        let system_message =
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(system_message_content),
                name: None,
            });

        // Also write a minimal default template
        let template = template::ChatTemplate {
            system_prompt:
                "You are Awful Jade, a helpful AI assistant programmed by Awful Security."
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
    pb.set_message("Writing default template...");
    create_default_template(&path, overwrite)?;

    // Baseline config file with local defaults
    let config_path = config_dir.join("config.yaml");

    if config_path.exists() && !overwrite {
        pb.set_message("Config file already exists (skipping)...");
        info!(
            "Config file already exists (skipping): {}",
            config_path.display()
        );
    } else {
        pb.set_message("Writing config file...");
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
        pb.set_message("Database already exists (skipping)...");
        info!(
            "Database file already exists (skipping): {}",
            db_path.display()
        );
    } else {
        pb.set_message("Creating database...");
        info!("Creating database file: {}", db_path.display());
        create_database(&db_path)?;
    }

    pb.finish_with_message("✓ Initialization complete");

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
fn create_default_template(
    templates_dir: &std::path::Path,
    overwrite: bool,
) -> Result<(), Box<dyn Error>> {
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

/// Process RAG documents and retrieve relevant context for the query.
///
/// This function:
/// 1. Parses the comma-separated list of file paths
/// 2. Reads each plain text file
/// 3. Creates a temporary VectorStore for RAG documents
/// 4. Intelligently chunks documents using tokenizer (512 tokens per chunk with 128 token overlap)
/// 5. Embeds the document chunks
/// 6. Retrieves the top-k most relevant chunks based on the query
/// 7. Returns concatenated relevant chunks as a single string
///
/// # Parameters
/// - `rag_files`: Comma-separated list of file paths
/// - `query`: The user's question to find relevant context for
///
/// # Returns
/// A string containing the concatenated relevant document chunks
///
/// # Errors
/// - I/O errors when reading files
/// - Vector store embedding/search errors
/// - Tokenizer errors
fn process_rag_documents(
    rag_files: &str,
    query: &str,
    top_k: usize,
) -> Result<String, Box<dyn Error>> {
    use hf_hub::{Repo, RepoType, api::sync::Api};
    use indicatif::{ProgressBar, ProgressStyle};
    use rayon::prelude::*;
    use std::time::Duration;
    use tokenizers::{Tokenizer, TruncationDirection, TruncationParams, TruncationStrategy};
    use tracing::{debug, info};

    // Spinner setup — single dynamic line
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")?
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message("RAG: preparing…");

    // Parse comma-separated paths
    let file_paths: Vec<&str> = rag_files
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if file_paths.is_empty() {
        pb.finish_with_message("RAG: no files provided");
        return Ok(String::new());
    }

    pb.set_message(format!("RAG: loading {} file(s)…", file_paths.len()));
    info!("RAG: Processing {} document(s)", file_paths.len());
    debug!("RAG: Document paths: {:?}", file_paths);

    // Load tokenizer (consistent with MiniLM-L6-v2)
    pb.set_message("RAG: loading tokenizer…");
    let model_id = "sentence-transformers/all-MiniLM-L6-v2";
    let repo = Repo::with_revision(model_id.to_string(), RepoType::Model, "main".to_string());
    let api = Api::new()?;
    let api_repo = api.repo(repo);
    let tokenizer_filename = api_repo.get("tokenizer.json")?;
    let mut tokenizer = Tokenizer::from_file(tokenizer_filename)
        .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

    // RAG store (384-dim)
    let mut rag_store = VectorStore::new(384, "rag_temp".to_string())?;

    // Chunking params
    let chunk_size = 512usize;
    let overlap = 128usize;
    info!(
        "RAG: Using chunk size of {} tokens with {} token overlap",
        chunk_size, overlap
    );

    // Sliding window via truncation + stride (no silent 128-cap)
    let _ = tokenizer.with_truncation(Some(TruncationParams {
        max_length: chunk_size,
        strategy: TruncationStrategy::LongestFirst,
        stride: overlap,
        direction: TruncationDirection::Right,
    }));

    // We’ll gather (text, vector) for all chunks across files,
    // mixing bincode cache hits & freshly embedded items.
    let mut all_chunks_with_vecs: Vec<(String, Vec<f32>)> = Vec::new();
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;

    for file_path in &file_paths {
        pb.set_message(format!("RAG: hashing '{}'…", file_path));
        let file_hash = hash_bytes_sha(file_path)?;

        if let Some(cache) = try_load_cache(&file_hash, model_id, chunk_size, overlap)? {
            // Cache hit: use cached chunks+vectors directly
            pb.set_message(format!(
                "RAG: cache hit ‘{}’ → {} chunks",
                file_path,
                cache.chunks.len()
            ));
            for c in cache.chunks {
                all_chunks_with_vecs.push((c.text, c.vector));
            }
            cache_hits += 1;
            continue;
        }

        cache_misses += 1;

        // No cache → read, tokenize, chunk, embed, then persist cache
        pb.set_message(format!("RAG: reading '{}'…", file_path));
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read RAG file {}: {}", file_path, e))?;
        debug!(
            "RAG: Read document '{}' ({} bytes)",
            file_path,
            content.len()
        );

        pb.set_message(format!("RAG: tokenizing '{}'…", file_path));
        let first = tokenizer
            .encode(content.clone(), true)
            .map_err(|e| format!("Failed to tokenize document '{}': {}", file_path, e))?;

        // First window + overflows
        let mut windows = Vec::with_capacity(1 + first.get_overflowing().len());
        windows.push(first.clone());
        windows.extend_from_slice(first.get_overflowing());

        let mut fresh_chunks_text: Vec<String> = Vec::new();
        for win in windows {
            let ids = win.get_ids(); // &[u32]
            if ids.is_empty() {
                continue;
            }
            let chunk_text = tokenizer
                .decode(ids, true)
                .map_err(|e| format!("Failed to decode chunk: {}", e))?;
            if chunk_text.trim().len() > 50 {
                fresh_chunks_text.push(chunk_text);
            }
        }
        debug!(
            "RAG: Extracted {} chunks from '{}'",
            fresh_chunks_text.len(),
            file_path
        );

        // Embed fresh chunks in parallel
        pb.set_message(format!(
            "RAG: embedding {} chunk(s) for '{}'…",
            fresh_chunks_text.len(),
            file_path
        ));
        // Embed fresh chunks in parallel — live progress on one line
        let total_file = fresh_chunks_text.len();
        let counter = AtomicUsize::new(0);
        let pb_file = pb.clone();
        pb_file.set_message(format!(
            "RAG: embedding 0/{total_file} chunk(s) for '{}'…",
            file_path
        ));

        let embedded: Vec<(String, Vec<f32>)> = fresh_chunks_text
            .par_iter()
            .map(|text| {
                let vec = rag_store
                    .embed_text_to_vector(text)
                    .unwrap_or_else(|e| panic!("Failed to embed '{}': {}", file_path, e));

                // Update the same spinner line with a done/total ratio
                let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                // Throttle updates a bit to avoid overwhelming the terminal
                if done % 50 == 0 || done == total_file {
                    pb_file.set_message(format!(
                        "RAG: embedding {done}/{total_file} chunk(s) for ‘{}’…",
                        file_path
                    ));
                }

                (text.clone(), vec)
            })
            .collect();

        // Save bincode cache
        let cache = RagCacheFile {
            version: 1,
            model_id: model_id.to_string(),
            chunk_size,
            overlap,
            file_hash: file_hash.clone(),
            created_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            chunks: embedded
                .iter()
                .map(|(t, v)| CachedChunk {
                    text: t.clone(),
                    vector: v.clone(),
                })
                .collect(),
        };
        save_cache(&cache)?;

        all_chunks_with_vecs.extend(embedded);
        pb.set_message(format!("RAG: cached '{}' ✓", file_path));
    }

    info!(
        "RAG: cache hits: {}, cache misses: {}",
        cache_hits, cache_misses
    );

    // Add everything to the store and keep vector list for distance calc
    pb.set_message("RAG: building index…");
    let mut rag_vectors: Vec<Vec<f32>> = Vec::with_capacity(all_chunks_with_vecs.len());
    for (text, vector) in all_chunks_with_vecs.into_iter() {
        let memory = awful_aj::brain::Memory::new(async_openai::types::Role::System, text);
        rag_store.add_vector_with_content(vector.clone(), memory)?;
        rag_vectors.push(vector);
    }
    rag_store.build()?;

    pb.set_message("RAG: searching…");
    let query_vector = rag_store.embed_text_to_vector(query)?;
    let neighbor_ids = rag_store.search(&query_vector, top_k)?;
    info!(
        "RAG: Retrieved {} relevant chunk(s) for query",
        neighbor_ids.len()
    );

    // Distance filtering without re-embedding
    let mut distances_and_content: Vec<(f32, String)> = Vec::with_capacity(neighbor_ids.len());
    for id in &neighbor_ids {
        if let Some(memory) = rag_store.get_content_by_id(*id) {
            if let Some(chunk_vector) = rag_vectors.get(*id) {
                let distance = VectorStore::calc_euclidean_distance(
                    query_vector.clone(),
                    chunk_vector.clone(),
                );
                distances_and_content.push((distance, memory.content.clone()));
            }
        }
    }

    let mut relevant_chunks = Vec::new();
    if !distances_and_content.is_empty() {
        let best = distances_and_content
            .iter()
            .map(|(d, _)| *d)
            .fold(f32::INFINITY, f32::min);
        let threshold = best * 1.10;
        for (d, c) in distances_and_content {
            if d <= threshold {
                relevant_chunks.push(c);
            }
        }
    }

    let context = relevant_chunks.join("\n\n---\n\n");
    pb.finish_with_message(format!("RAG: ready ✓ ({} chars of context)", context.len()));
    Ok(context)
}

/// Resolve the per-user configuration directory.
///
/// Uses [`directories::ProjectDirs`] with the tuple `("com", "awful-sec", "aj")`
/// to compute an OS-appropriate configuration directory:
///
/// - **macOS**: `~/Library/Application Support/com.awful-sec.aj`
/// - **Linux**: `~/.config/aj`
/// - **Windows**: `%APPDATA%\\awful-sec\\aj`
///
/// This location is used for `config.yaml`, the `templates/` folder, the
/// per-session vector store YAMLs, and the downloaded `all-MiniLM-L6-v2` model.
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
