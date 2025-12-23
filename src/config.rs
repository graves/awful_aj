//! # Configuration Management for Awful Jade
//!
//! This module provides configuration loading, validation, and database synchronization
//! for the Awful Jade application. Configuration is stored in YAML format and can be
//! optionally synced to the SQLite database for session-specific settings tracking.
//!
//! ## Overview
//!
//! The central type is [`AwfulJadeConfig`], which encapsulates all runtime settings:
//!
//! - **API Configuration**: Endpoint URL, API key, model name
//! - **Token Budgeting**: Context limits and minimum assistant tokens
//! - **Generation Control**: Stop words and streaming preferences
//! - **Session Management**: Database path and active session name
//!
//! ## Configuration Loading
//!
//! Configuration is loaded from a YAML file in the platform-specific config directory
//! (see [`crate::config_dir()`]). Use [`load_config()`] to read and deserialize the file.
//!
//! ### Typical Workflow
//!
//! 1. **Load config** from YAML using [`load_config()`]
//! 2. **Validate settings** (automatic during deserialization)
//! 3. **Sync to database** (optional) using [`AwfulJadeConfig::ensure_conversation_and_config()`]
//! 4. **Use throughout application** for API calls, token budgeting, etc.
//!
//! ## YAML Configuration Format
//!
//! ```yaml
//! # OpenAI-compatible API settings
//! api_key: "sk-...or-empty-for-local-backend..."
//! api_base: "http://localhost:5001/v1"
//! model: "qwen2.5-7b-instruct"
//!
//! # Token budgeting
//! context_max_tokens: 8192
//! assistant_minimum_context_tokens: 2048
//!
//! # Generation control
//! stop_words: ["<|im_end|>", "<|im_start|>"]
//! should_stream: true
//!
//! # Database and session
//! session_db_url: "/path/to/aj.db"
//! session_name: "default"  # optional
//! ```
//!
//! ## Configuration Paths by Platform
//!
//! | Platform | Config Path |
//! |----------|-------------|
//! | **macOS** | `~/Library/Application Support/com.awful-sec.aj/config.yaml` |
//! | **Linux** | `~/.config/aj/config.yaml` |
//! | **Windows** | `%APPDATA%\com.awful-sec\aj\config.yaml` |
//!
//! ## Database Synchronization
//!
//! When using named sessions, you can persist configuration snapshots to the database
//! using [`AwfulJadeConfig::ensure_conversation_and_config()`]. This:
//!
//! 1. Creates or retrieves a `conversations` row for the session name
//! 2. Inserts an `awful_configs` snapshot if settings have changed
//! 3. Links the config to the conversation for historical tracking
//!
//! This enables auditing which settings were active for past conversations.
//!
//! ## Examples
//!
//! ### Loading Configuration
//!
//! ```no_run
//! use awful_aj::config::load_config;
//! use awful_aj::config_dir;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load from default location
//! let config_path = config_dir()?.join("config.yaml");
//! let config = load_config(config_path.to_str().unwrap())?;
//!
//! println!("API Base: {}", config.api_base);
//! println!("Model: {}", config.model);
//! println!("Context tokens: {}", config.context_max_tokens);
//! # Ok(())
//! # }
//! ```
//!
//! ### Session Configuration Sync
//!
//! ```no_run
//! use awful_aj::config::AwfulJadeConfig;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Minimal config for demonstration
//! let mut cfg = AwfulJadeConfig {
//!     api_key: "sk-test".into(),
//!     api_base: "http://localhost:5001/v1".into(),
//!     model: "qwen2.5-7b".into(),
//!     context_max_tokens: 8192,
//!     assistant_minimum_context_tokens: 2048,
//!     stop_words: vec!["<|im_end|>".into()],
//!     session_db_url: "aj.db".into(),
//!     session_name: None,
//!     should_stream: Some(true),
//!     temperature: None,
//! };
//!
//! // Sync to database for session tracking
//! cfg.ensure_conversation_and_config("my-research-session").await?;
//! println!("Session: {}", cfg.session_name.unwrap());
//! # Ok(())
//! # }
//! ```
//!
//! ### Programmatic Configuration
//!
//! ```no_run
//! use awful_aj::config::AwfulJadeConfig;
//!
//! // Create configuration programmatically (for testing or library use)
//! let config = AwfulJadeConfig {
//!     api_key: String::new(), // Empty for local backends
//!     api_base: "http://localhost:11434/v1".into(),
//!     model: "llama3.2:latest".into(),
//!     context_max_tokens: 4096,
//!     assistant_minimum_context_tokens: 1024,
//!     stop_words: vec![],
//!     session_db_url: "memory.db".into(),
//!     session_name: Some("test-session".into()),
//!     should_stream: Some(false),
//!     temperature: None,
//! };
//! ```
//!
//! ## See Also
//!
//! - [`crate::config_dir()`] - Get platform-specific config directory
//! - [`crate::models::AwfulConfig`] - Database ORM model for persisted configs
//! - [`crate::models::Conversation`] - Database model for sessions
use crate::models::*;
use diesel::prelude::*;

use serde::{Deserialize, Serialize};
use std::{error::Error, fs};

use tracing::*;

/// In-memory application configuration loaded from `config.yaml`.
///
/// This struct represents the complete runtime configuration for Awful Jade,
/// controlling API interactions, token budgeting, generation behavior, and
/// session management.
///
/// # Configuration Sections
///
/// ## API Configuration
/// - [`api_key`](Self::api_key): Authentication for the LLM API (optional for local backends)
/// - [`api_base`](Self::api_base): OpenAI-compatible endpoint URL
/// - [`model`](Self::model): Model identifier to request
///
/// ## Token Budgeting
/// - [`context_max_tokens`](Self::context_max_tokens): Maximum total context window size
/// - [`assistant_minimum_context_tokens`](Self::assistant_minimum_context_tokens): Reserved tokens for response
///
/// ## Generation Control
/// - [`stop_words`](Self::stop_words): Termination sequences for generation
/// - [`should_stream`](Self::should_stream): Enable token-by-token streaming
///
/// ## Session Management
/// - [`session_db_url`](Self::session_db_url): SQLite database file path
/// - [`session_name`](Self::session_name): Active conversation identifier
///
/// # Serialization
///
/// The struct implements [`Serialize`] and [`Deserialize`] for YAML persistence.
/// Use [`load_config()`] to deserialize from a file.
///
/// # Examples
///
/// ## Creating a Configuration
///
/// ```rust
/// use awful_aj::config::AwfulJadeConfig;
///
/// let config = AwfulJadeConfig {
///     api_key: String::new(),  // Empty for local LLMs
///     api_base: "http://localhost:11434/v1".into(),
///     model: "llama3.2:latest".into(),
///     context_max_tokens: 8192,
///     assistant_minimum_context_tokens: 2048,
///     stop_words: vec!["<|eot_id|>".into()],
///     session_db_url: "~/data/aj.db".into(),
///     session_name: Some("default".into()),
///     should_stream: Some(true),
///     temperature: None,
/// };
/// ```
///
/// ## Loading from YAML
///
/// ```no_run
/// use awful_aj::config::load_config;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = load_config("/path/to/config.yaml")?;
/// println!("Loaded config for model: {}", config.model);
/// # Ok(())
/// # }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AwfulJadeConfig {
    /// API key for authentication with the LLM endpoint.
    ///
    /// This field is passed to the OpenAI client for bearer token authentication.
    ///
    /// **Local backends**: Leave empty or use a placeholder value (e.g., `"LOCAL"`)
    /// for unsecured local LLM servers like Ollama or LM Studio.
    ///
    /// **Cloud providers**: Use your actual API key (e.g., `"sk-..."` for OpenAI).
    ///
    /// # Security
    ///
    /// Store this configuration file with appropriate file permissions (e.g., `600`
    /// on Unix) to prevent unauthorized access to API keys.
    pub api_key: String,

    /// Base URL of the OpenAI-compatible API endpoint.
    ///
    /// Must include the version path (e.g., `/v1`). Examples:
    ///
    /// - **Ollama**: `http://localhost:11434/v1`
    /// - **LM Studio**: `http://localhost:1234/v1`
    /// - **vLLM**: `http://your-server:8000/v1`
    /// - **OpenAI**: `https://api.openai.com/v1`
    ///
    /// # Notes
    ///
    /// The URL must be reachable from the machine running Awful Jade. For remote
    /// servers, ensure firewall rules allow the connection.
    pub api_base: String,

    /// Model identifier to request from the API.
    ///
    /// The exact value depends on your backend:
    ///
    /// - **Ollama**: `"llama3.2:latest"`, `"mistral:7b"`
    /// - **LM Studio**: Whatever name you've loaded (e.g., `"qwen2.5-7b-instruct"`)
    /// - **OpenAI**: `"gpt-4o"`, `"gpt-3.5-turbo"`
    ///
    /// This string is passed directly to the `/v1/chat/completions` endpoint.
    pub model: String,

    /// Maximum tokens for the entire context window.
    ///
    /// This includes:
    /// - System prompt (preamble)
    /// - RAG context chunks
    /// - Conversation history (memories)
    /// - Current user prompt
    /// - Reserved space for assistant response
    ///
    /// **Token budgeting**: When the context exceeds this limit, the [`Brain`](crate::brain::Brain)
    /// evicts the oldest memories using a FIFO strategy.
    ///
    /// # Typical Values
    ///
    /// - **4k models**: `4096`
    /// - **8k models**: `8192`
    /// - **16k models**: `16384`
    /// - **32k models**: `32768`
    /// - **128k models**: `131072`
    ///
    /// Set this to match your model's actual context window to avoid truncation errors.
    pub context_max_tokens: usize,

    /// Minimum tokens to reserve for the assistant's response.
    ///
    /// This value is subtracted from [`context_max_tokens`](Self::context_max_tokens)
    /// when budgeting the prompt. It ensures the model has enough room to generate
    /// a complete response.
    ///
    /// # Typical Values
    ///
    /// - **Short answers**: `512` - `1024`
    /// - **Medium responses**: `2048`
    /// - **Long-form generation**: `4096`
    ///
    /// **Example**: If `context_max_tokens = 8192` and `assistant_minimum_context_tokens = 2048`,
    /// the prompt must fit within `8192 - 2048 = 6144` tokens.
    pub assistant_minimum_context_tokens: i32,

    /// Stop sequences to terminate generation.
    ///
    /// When the model generates any of these strings, generation halts immediately.
    /// This is useful for:
    ///
    /// - Preventing the model from continuing past its intended response
    /// - Enforcing chat template boundaries (e.g., ChatML's `<|im_end|>`)
    /// - Implementing custom termination logic
    ///
    /// # Examples
    ///
    /// ```yaml
    /// stop_words:
    ///   - "<|im_end|>"
    ///   - "<|im_start|>"
    ///   - "\n\nHuman:"
    /// ```
    ///
    /// **Note**: Some models ignore stop words or only support a limited number.
    /// Check your backend's documentation.
    pub stop_words: Vec<String>,

    /// SQLite database file path for conversation persistence.
    ///
    /// This is where all sessions, messages, and configuration snapshots are stored.
    ///
    /// # Path Resolution
    ///
    /// - **Absolute paths**: Used as-is (e.g., `/Users/alice/data/aj.db`)
    /// - **Relative paths**: Resolved from the current working directory
    /// - **Empty string**: Auto-set to default location in config directory
    ///
    /// # Default Location
    ///
    /// If left empty, [`load_config()`] sets this to:
    ///
    /// - **macOS**: `~/Library/Application Support/com.awful-sec.aj/aj.db`
    /// - **Linux**: `~/.config/aj/aj.db`
    /// - **Windows**: `%APPDATA%\com.awful-sec\aj\aj.db`
    ///
    /// # Database Schema
    ///
    /// See [`crate::schema`] for table definitions.
    pub session_db_url: String,

    /// Active session/conversation name.
    ///
    /// When set, all interactions are linked to this conversation in the database,
    /// enabling:
    ///
    /// - Context continuity across multiple CLI invocations
    /// - Vector search over past conversation turns
    /// - Session-specific configuration tracking
    ///
    /// # Usage
    ///
    /// - **`None`**: One-shot mode, no persistence
    /// - **`Some("session-name")`**: Named session mode
    ///
    /// This value can be overridden via CLI flags (`-s/--session`).
    pub session_name: Option<String>,

    /// Enable streaming token-by-token responses.
    ///
    /// When `Some(true)`, the application:
    ///
    /// 1. Calls the `/v1/chat/completions` endpoint with `stream: true`
    /// 2. Processes Server-Sent Events (SSE) as they arrive
    /// 3. Prints tokens to stdout in real-time
    /// 4. (If `--pretty` is enabled) Replaces output with formatted version
    ///
    /// When `Some(false)` or `None`:
    ///
    /// 1. Waits for the complete response
    /// 2. Prints the entire message at once
    ///
    /// # User Experience
    ///
    /// Streaming provides immediate feedback and feels more interactive, but
    /// non-streaming can be faster for short responses or slow connections.
    ///
    /// # Compatibility
    ///
    /// All OpenAI-compatible backends should support streaming, but some
    /// local setups may have issues. If streaming fails, Awful Jade
    /// automatically falls back to non-streaming.
    pub should_stream: Option<bool>,

    /// Sampling temperature for response generation.
    ///
    /// Controls the randomness of the model's output:
    ///
    /// - **0.0**: Deterministic, always picks the most likely token
    /// - **0.7**: Balanced creativity and coherence (recommended default)
    /// - **1.0**: More creative/random responses
    /// - **>1.0**: Increasingly random (may produce incoherent output)
    ///
    /// # Default Value
    ///
    /// When `None`, defaults to `0.7` which works well for most coding tasks.
    ///
    /// # Example
    ///
    /// ```yaml
    /// temperature: 0.7  # Balanced (default)
    /// temperature: 0.0  # Deterministic
    /// temperature: 1.0  # Creative
    /// ```
    #[serde(default)]
    pub temperature: Option<f32>,
}

impl AwfulJadeConfig {
    /// Ensures a conversation and configuration snapshot exist in the database for the given session.
    ///
    /// This method performs database synchronization to track which configuration was
    /// active for a particular conversation. It's useful for auditing and reproducing
    /// past sessions with their exact settings.
    ///
    /// # What It Does
    ///
    /// Within a single database transaction:
    ///
    /// 1. **Find or create conversation**:
    ///    - Searches for an existing `conversations` row with `session_name == a_session_name`
    ///    - If not found, inserts a new conversation row
    ///
    /// 2. **Find or update config snapshot**:
    ///    - Searches for an `awful_configs` row linked to the conversation
    ///    - If none exists, or if the stored settings differ from current `self`, inserts a new snapshot
    ///    - Comparison uses [`PartialEq<AwfulJadeConfig>`](PartialEq) implementation
    ///
    /// 3. **Update session name**:
    ///    - Sets `self.session_name = Some(a_session_name.to_string())`
    ///
    /// # When to Call
    ///
    /// Typically called once during application initialization when running in named
    /// session mode:
    ///
    /// - Interactive mode: `aj interactive -s my-session`
    /// - Ask with session: `aj ask -s my-session "question"`
    ///
    /// Not needed for one-shot queries without persistence.
    ///
    /// # Parameters
    ///
    /// - `a_session_name`: Friendly name to identify the conversation (e.g., `"project-refactor"`, `"debug-auth"`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Database connection fails
    /// - Transaction fails (e.g., constraint violation)
    /// - Diesel query errors occur
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use awful_aj::config::AwfulJadeConfig;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut config = AwfulJadeConfig {
    ///     api_key: "key".into(),
    ///     api_base: "http://localhost:5001/v1".into(),
    ///     model: "qwen2".into(),
    ///     context_max_tokens: 8192,
    ///     assistant_minimum_context_tokens: 2048,
    ///     stop_words: vec![],
    ///     session_db_url: "aj.db".into(),
    ///     session_name: None,
    ///     should_stream: Some(true),
    ///     temperature: None,
    /// };
    ///
    /// // Sync to database for "my-research" session
    /// config.ensure_conversation_and_config("my-research").await?;
    ///
    /// assert_eq!(config.session_name, Some("my-research".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Implementation Details
    ///
    /// The comparison between stored config and current config checks:
    /// - `api_base`
    /// - `api_key`
    /// - `model`
    /// - `context_max_tokens`
    /// - `assistant_minimum_context_tokens`
    ///
    /// If any differ, a new snapshot is inserted. This creates an audit trail
    /// of configuration changes over the lifetime of a session.
    ///
    /// # See Also
    ///
    /// - [`crate::models::Conversation`] - Database model for conversations
    /// - [`crate::models::AwfulConfig`] - Database model for config snapshots
    pub async fn ensure_conversation_and_config(
        &mut self,
        a_session_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut connection = establish_connection(&self.session_db_url);

        // Begin a new transaction
        connection.transaction(|conn| {
            // Check for an existing conversation with the specified session name
            let existing_conversation = crate::schema::conversations::table
                .filter(crate::schema::conversations::session_name.eq(a_session_name))
                .first(conn)
                .optional()?;

            info!("EXISTING CONVERSATION: {:?}", existing_conversation);

            // If conversation doesn't exist, create a new one
            let conversation = if let Some(conversation) = existing_conversation {
                conversation
            } else {
                let new_conversation = Conversation {
                    id: None,
                    session_name: a_session_name.to_string(),
                };
                diesel::insert_into(crate::schema::conversations::table)
                    .values(&new_conversation)
                    .returning(Conversation::as_returning())
                    .get_result(conn)
                    .expect("Error saving new Conversation!")
            };

            info!("CONVERSATION: {:?}", conversation);

            // Check for an existing config associated with this conversation
            let existing_config: Option<AwfulConfig> = crate::schema::awful_configs::table
                .filter(crate::schema::awful_configs::conversation_id.eq(conversation.id))
                .first(conn)
                .optional()?;

            info!("EXISTING CONFIG: {:?}", existing_config);

            // If config doesn't exist or differs, create a new one
            if existing_config.is_none() || existing_config.unwrap() != *self {
                let new_config = AwfulConfig {
                    id: None,
                    conversation_id: Some(conversation.id().expect("Conversation has no ID!")),
                    api_key: self.api_key.clone(),
                    api_base: self.api_base.clone(),
                    model: self.model.clone(),
                    context_max_tokens: self.context_max_tokens as i32,
                    assistant_minimum_context_tokens: self.assistant_minimum_context_tokens as i32,
                    stop_words: self.stop_words.join(","),
                };
                diesel::insert_into(crate::schema::awful_configs::table)
                    .values(&new_config)
                    .execute(conn)?;
            }

            self.session_name = Some(a_session_name.to_string());

            Ok(())
        })
    }
}

/// Loads configuration from a YAML file and validates/normalizes settings.
///
/// This function reads a YAML configuration file, deserializes it into an
/// [`AwfulJadeConfig`] struct, and performs automatic validation and normalization:
///
/// - If `session_db_url` is empty, sets it to the default location in the config directory
/// - Trims whitespace from the database URL
/// - Logs warnings for empty/invalid paths
///
/// # Parameters
///
/// - `file`: Path to the YAML configuration file (absolute or relative)
///
/// # Returns
///
/// - `Ok(AwfulJadeConfig)`: Successfully loaded and validated configuration
/// - `Err(Box<dyn Error>)`: File doesn't exist, can't be read, or contains invalid YAML
///
/// # Errors
///
/// This function returns an error if:
///
/// - The file doesn't exist or can't be read (I/O error)
/// - The file contains invalid YAML syntax (parse error)
/// - Required fields are missing from the YAML
/// - Field types don't match expected types (e.g., string instead of number)
///
/// # Configuration File Format
///
/// The YAML file should contain all required fields with appropriate types:
///
/// ```yaml
/// api_key: "your-api-key-or-empty"
/// api_base: "http://localhost:5001/v1"
/// model: "model-name"
/// context_max_tokens: 8192
/// assistant_minimum_context_tokens: 2048
/// stop_words: ["<|im_end|>"]
/// session_db_url: "/path/to/aj.db"  # Optional, auto-filled if empty
/// session_name: null  # or "session-name"
/// should_stream: true  # or false
/// ```
///
/// # Default Database Path
///
/// If `session_db_url` is empty or contains only whitespace, it's automatically set to:
///
/// - **macOS**: `~/Library/Application Support/com.awful-sec.aj/aj.db`
/// - **Linux**: `~/.config/aj/aj.db`
/// - **Windows**: `%APPDATA%\com.awful-sec\aj\aj.db`
///
/// # Examples
///
/// ## Basic Usage
///
/// ```no_run
/// use awful_aj::config::load_config;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = load_config("/path/to/config.yaml")?;
///
/// println!("API Base: {}", config.api_base);
/// println!("Model: {}", config.model);
/// println!("Context tokens: {}", config.context_max_tokens);
/// # Ok(())
/// # }
/// ```
///
/// ## Loading from Default Location
///
/// ```no_run
/// use awful_aj::{config::load_config, config_dir};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config_path = config_dir()?.join("config.yaml");
/// let config = load_config(config_path.to_str().unwrap())?;
///
/// println!("Loaded config from default location");
/// # Ok(())
/// # }
/// ```
///
/// ## Error Handling
///
/// ```no_run
/// use awful_aj::config::load_config;
///
/// match load_config("/path/to/config.yaml") {
///     Ok(config) => println!("Loaded config for model: {}", config.model),
///     Err(e) => eprintln!("Failed to load config: {}", e),
/// }
/// ```
///
/// # See Also
///
/// - [`AwfulJadeConfig`] - Configuration struct definition
/// - [`crate::config_dir()`] - Get platform-specific config directory
/// - [`crate::commands::Commands::Init`] - Create default configuration file
pub fn load_config(file: &str) -> Result<AwfulJadeConfig, Box<dyn Error>> {
    let content = fs::read_to_string(file)?;
    let mut config: AwfulJadeConfig = serde_yaml::from_str(&content)?;

    // Validate and normalize the database path
    if config.session_db_url.trim().is_empty() {
        warn!("session_db_url is empty, using default path in config directory");
        let default_db_path = crate::config_dir()?.join("aj.db");
        config.session_db_url = default_db_path.to_string_lossy().to_string();
        info!("Database path set to: {}", config.session_db_url);
    }

    Ok(config)
}

/// Establishes a SQLite database connection using Diesel.
///
/// This function creates a connection to the SQLite database at the specified path.
/// It's used throughout the application for all database operations (sessions,
/// messages, configuration snapshots).
///
/// # Parameters
///
/// - `db_url`: File path to the SQLite database (e.g., `"/Users/alice/aj.db"` or `"memory.db"`)
///
/// # Returns
///
/// A [`SqliteConnection`] ready for use with Diesel queries.
///
/// # Panics
///
/// This function **panics** if the connection cannot be established. This design
/// is intentional for CLI applications where database connectivity is critical
/// and early failure is preferable to partial functionality.
///
/// Panic occurs if:
/// - Database file doesn't exist and SQLite can't create it
/// - File permissions prevent access
/// - Database file is corrupted
/// - Disk is full (can't create lock file)
///
/// # Usage Notes
///
/// ## CLI Applications (Recommended)
///
/// For command-line tools, panicking on connection failure provides clear error
/// messages and prevents the application from continuing in an invalid state:
///
/// ```no_run
/// use awful_aj::config::establish_connection;
///
/// let conn = establish_connection("/path/to/aj.db");
/// // Use conn for queries...
/// ```
///
/// ## Long-Running Services (Alternative Pattern)
///
/// For servers or daemons that should recover from transient failures, consider
/// wrapping this in a retry loop or using Diesel's connection pooling:
///
/// ```no_run
/// use diesel::prelude::*;
///
/// fn establish_connection_with_retry(db_url: &str) -> Result<SqliteConnection, String> {
///     SqliteConnection::establish(db_url)
///         .map_err(|e| format!("Failed to connect to {}: {}", db_url, e))
/// }
/// ```
///
/// # Examples
///
/// ## Basic Connection
///
/// ```no_run
/// use awful_aj::config::establish_connection;
/// use diesel::prelude::*;
///
/// let mut conn = establish_connection("aj.db");
///
/// // Now use conn for queries
/// ```
///
/// ## With Configuration
///
/// ```no_run
/// use awful_aj::config::{load_config, establish_connection};
/// use awful_aj::config_dir;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config_path = config_dir()?.join("config.yaml");
/// let config = load_config(config_path.to_str().unwrap())?;
///
/// let mut conn = establish_connection(&config.session_db_url);
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`AwfulJadeConfig::session_db_url`] - Configuration field for database path
/// - [`crate::session_messages::SessionMessages`] - Uses this function for database access
pub fn establish_connection(db_url: &str) -> SqliteConnection {
    SqliteConnection::establish(db_url).unwrap_or_else(|_| panic!("Error connecting to {}", db_url))
}

/// Compares a database-persisted configuration snapshot with an in-memory configuration.
///
/// This custom [`PartialEq`] implementation enables comparing a [`crate::models::AwfulConfig`]
/// (database ORM model) with an [`AwfulJadeConfig`] (in-memory YAML config) to determine
/// if configuration has changed.
///
/// # Comparison Strategy
///
/// The implementation compares the following fields:
///
/// - `api_base`: Must match exactly
/// - `api_key`: Must match exactly
/// - `model`: Must match exactly
/// - `context_max_tokens`: Compared after integer type conversion
/// - `assistant_minimum_context_tokens`: Must match exactly
///
/// **Not compared**:
/// - `stop_words`: Stored as comma-separated string in DB, complex to compare
/// - `should_stream`: Not persisted to database
/// - `session_name`: Not part of config snapshot
/// - `session_db_url`: Not stored in config snapshot
///
/// # Use Case
///
/// This is primarily used by [`AwfulJadeConfig::ensure_conversation_and_config()`]
/// to decide whether to insert a new configuration snapshot:
///
/// ```text
/// if existing_config.is_none() || existing_config.unwrap() != *self {
///     // Insert new config snapshot
/// }
/// ```
///
/// This creates an audit trail of configuration changes over time.
///
/// # Examples
///
/// ```rust
/// use awful_aj::config::AwfulJadeConfig;
/// use awful_aj::models::AwfulConfig;
///
/// let in_memory_config = AwfulJadeConfig {
///     api_key: "key".into(),
///     api_base: "http://localhost:5001/v1".into(),
///     model: "qwen2".into(),
///     context_max_tokens: 8192,
///     assistant_minimum_context_tokens: 2048,
///     stop_words: vec![],
///     session_db_url: "aj.db".into(),
///     session_name: None,
///     should_stream: Some(true),
///     temperature: None,
/// };
///
/// let db_config = AwfulConfig {
///     id: Some(1),
///     conversation_id: Some(42),
///     api_key: "key".into(),
///     api_base: "http://localhost:5001/v1".into(),
///     model: "qwen2".into(),
///     context_max_tokens: 8192,
///     assistant_minimum_context_tokens: 2048,
///     stop_words: "".into(),
/// };
///
/// // Configs match (stop_words not compared)
/// assert_eq!(db_config, in_memory_config);
/// ```
///
/// # Implementation Notes
///
/// The `context_max_tokens` field requires a cast from `i32` (database) to `usize`
/// (in-memory config) due to Diesel's type mapping for SQLite integers.
///
/// # See Also
///
/// - [`AwfulJadeConfig::ensure_conversation_and_config()`] - Primary consumer of this comparison
/// - [`crate::models::AwfulConfig`] - Database ORM model
impl PartialEq<AwfulJadeConfig> for AwfulConfig {
    fn eq(&self, other: &AwfulJadeConfig) -> bool {
        self.api_base == other.api_base
            && self.api_key == other.api_key
            && self.model == other.model
            && self.context_max_tokens as usize == other.context_max_tokens
            && self.assistant_minimum_context_tokens == other.assistant_minimum_context_tokens
    }
}

#[cfg(test)]
mod tests {
    use crate::config_dir;

    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Verifies that a well-formed YAML file loads into `AwfulJadeConfig`.
    #[test]
    fn test_load_config_valid_file() {
        // Create a temporary file with a valid configuration.
        let config_dir = config_dir().expect("Config directory doesnt exist");
        let mut temp_file = NamedTempFile::new_in(config_dir).unwrap();
        writeln!(
            temp_file,
            r#"
api_key: "example_api_key"
api_base: "http://example.com"
session_db_url: "aj.db"
model: "example_model"
context_max_tokens: 8192
assistant_minimum_context_tokens: 2048
stop_words: ["<|im_end|>", "\n"]
"#
        )
        .unwrap();

        // Load the configuration from the temporary file.
        let config = load_config(temp_file.path().to_str().unwrap());

        // Assert that the configuration was loaded successfully and has the expected values.
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.api_key, "example_api_key");
        assert_eq!(config.api_base, "http://example.com");
        assert_eq!(config.session_db_url, "aj.db");
        assert_eq!(config.model, "example_model");
        assert_eq!(config.context_max_tokens, 8192);
        assert_eq!(config.assistant_minimum_context_tokens, 2048);
    }

    /// Non-existent file should surface an error.
    #[test]
    fn test_load_config_invalid_file() {
        let config = load_config("non/existent/path");
        assert!(config.is_err());
    }

    /// Malformed YAML should fail to parse.
    #[test]
    fn test_load_config_invalid_format() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, r#"invalid: config: format"#).unwrap();

        let config = load_config(temp_file.path().to_str().unwrap());
        assert!(config.is_err());
    }
}
