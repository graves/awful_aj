//! # Configuration module
//!
//! This module defines the runtime configuration for Awful Jade and the helpers
//! to **load** it from YAML and **sync** it with the session database.
//!
//! The central type is [`AwfulJadeConfig`], which is deserialized from a
//! `config.yaml` you manage in the user’s config directory (see `lib.rs`’s
//! [`crate::config_dir`]). For named conversations, you can call
//! [`AwfulJadeConfig::ensure_conversation_and_config`] to create or link a
//! conversation row and persist a snapshot of the config in the DB.
//!
//! ## Typical flow
//! 1. Load config from YAML via [`load_config`].
//! 2. If running in a named session, call
//!    [`AwfulJadeConfig::ensure_conversation_and_config`] with the
//!    session name; this guarantees a `conversations` row and (if needed)
//!    an `awful_configs` row that mirrors the current in-memory config.
//! 3. Proceed to build prompts / issue API requests using these settings.
//!
//! ## YAML shape
//! ```yaml
//! api_key: "sk-...or-empty-for-local-backend..."
//! api_base: "http://localhost:5001/v1"
//! model: "qwen-or-your-favorite"
//! context_max_tokens: 8192
//! assistant_minimum_context_tokens: 2048
//! stop_words: ["\n<|im_start|>", "<|im_end|>"]
//! session_db_url: "/absolute/or/relative/path/to/aj.db"
//! # Optional:
//! session_name: "marketing-demo"
//! should_stream: true
//! ```
//!
/// # Examples
///
/// ```no_run
/// use awful_aj::config::AwfulJadeConfig;
///
/// // Minimal config just for illustration:
/// let mut cfg = AwfulJadeConfig {
///     api_key: "KEY".into(),
///     api_base: "http://localhost:5001/v1".into(),
///     model: "gpt-4o".into(),
///     context_max_tokens: 8192,
///     assistant_minimum_context_tokens: 2048,
///     stop_words: vec![],
///     session_db_url: "aj.db".into(),
///     session_name: None,
///     should_stream: Some(false),
/// };
///
/// // Drive the async call from a tiny runtime inside the doctest:
/// let rt = tokio::runtime::Runtime::new().unwrap();
/// rt.block_on(async {
///     cfg.ensure_conversation_and_config("my-session-name").await.unwrap();
/// });
/// ```
use crate::models::*;
use diesel::prelude::*;

use serde::{Deserialize, Serialize};
use std::{error::Error, fs};

use tracing::*;

/// In-memory application settings loaded from `config.yaml`.
///
/// These values control **how** Awful Jade talks to your OpenAI-compatible
/// backend (base URL, key, model), how large the assistant’s context can be
/// (`context_max_tokens`), where the message history DB lives (`session_db_url`),
/// whether to **stream** responses, and (optionally) which **named session**
/// is active.
///
/// The struct is [`Serialize`]/[`Deserialize`] and meant to be read from YAML via
/// [`load_config`].
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AwfulJadeConfig {
    /// API key used by the OpenAI client. Leave empty for unsecured local backends.
    pub api_key: String,

    /// Base URL of the OpenAI-compatible API, e.g. `http://localhost:5001/v1`.
    pub api_base: String,

    /// The model identifier to request (e.g., `qwen2`, `mistral-7b`, etc.).
    pub model: String,

    /// Maximum tokens for a single model response. Used to cap completions.
    pub context_max_tokens: usize,

    /// A minimum context that should remain available for the assistant
    /// (used by higher-level logic to budget prompt vs. reply).
    pub assistant_minimum_context_tokens: i32,

    /// Stop strings to terminate generation early.
    pub stop_words: Vec<String>,

    /// SQLite database URL (file path) where conversations/messages are stored.
    pub session_db_url: String,

    /// Optional name of the active conversation/session.
    pub session_name: Option<String>,

    /// If `Some(true)`, stream assistant tokens to stdout in `ask`.
    /// If `Some(false)` or `None`, perform non-streaming requests.
    pub should_stream: Option<bool>,
}

impl AwfulJadeConfig {
    /// Ensure a conversation and a persisted config snapshot exist for `a_session_name`.
    ///
    /// This function performs a single transaction that:
    /// 1. Looks up an existing `conversations.session_name == a_session_name`.
    ///    If none is found, it **inserts** a new conversation row.
    /// 2. Checks for an `awful_configs` row linked to that conversation.
    ///    If none exists **or** the stored settings differ from `self`
    ///    (see the custom `PartialEq` impl with `AwfulConfig`), it **inserts**
    ///    a new config snapshot row.
    /// 3. Sets `self.session_name = Some(a_session_name.into())`.
    ///
    /// This is typically called once after reading the YAML config if you
    /// are running a named session (interactive mode or `ask --session ...`).
    ///
    /// # Parameters
    /// - `a_session_name`: The friendly name to identify this conversation.
    ///
    /// # Errors
    /// Propagates Diesel errors if the transaction fails and bubbles up any other
    /// DB or I/O issues as a boxed error.
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

/// Load configuration from a YAML file on disk.
///
/// This is a thin convenience wrapper over `fs::read_to_string` + `serde_yaml::from_str`.
///
/// # Parameters
/// - `file`: Path to a YAML config file.
///
/// # Returns
/// - `Ok(AwfulJadeConfig)` on success.
/// - `Err(..)` if the file can’t be read or cannot be parsed as valid YAML.
///
/// # Examples
/// ```no_run
/// use awful_aj::config::load_config;
///
/// let cfg = load_config("/path/to/config.yaml")?;
/// assert!(!cfg.api_base.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_config(file: &str) -> Result<AwfulJadeConfig, Box<dyn Error>> {
    let content = fs::read_to_string(file)?;
    let config: AwfulJadeConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Establish a Diesel `SqliteConnection` to the session database.
///
/// Panics with a clear message if the connection cannot be opened.
/// Prefer this for small CLI tools where early-exit is acceptable; for
/// long-running services you might want to return a `Result` instead.
pub fn establish_connection(db_url: &str) -> SqliteConnection {
    SqliteConnection::establish(db_url).unwrap_or_else(|_| panic!("Error connecting to {}", db_url))
}

/// Compare a persisted DB snapshot (`AwfulConfig`) with an in-memory [`AwfulJadeConfig`].
///
/// This drives the “should we insert a new `awful_configs` row?” decision in
/// [`AwfulJadeConfig::ensure_conversation_and_config`]. It compares API base,
/// key, model, `context_max_tokens` (with integer cast), and
/// `assistant_minimum_context_tokens`. If any differ, a new snapshot is inserted.
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
