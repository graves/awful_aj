//! This module provides functionality for loading and handling the application's configuration.
//!
//! It defines the `AwfulJadeConfig` struct, which holds the configuration parameters,
//! and a `load_config` function to load the configuration from a file.
//!
//! # Examples
//!
//! Loading the configuration from a file:
//!
//! ```no_run
//! use awful_jade::config::{AwfulJadeConfig, load_config};
//!
//! let config_file_path = "/path/to/config.yaml";
//! let config: AwfulJadeConfig = load_config(config_file_path).unwrap();
//! println!("{:?}", config);
//! ```

use crate::models::*;
use diesel::prelude::*;

use serde::{Deserialize, Serialize};
use std::{error::Error, fs};

use tracing::*;

/// Represents the application's configuration.
///
/// This struct holds the configuration parameters needed to run the application,
/// such as API key, API base URL, and model name. It can be constructed by loading
/// a YAML configuration file using the `load_config` function.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AwfulJadeConfig {
    /// The API key used to authenticate requests to the API.
    pub api_key: String,

    /// The base URL of the API.
    pub api_base: String,

    /// The name of the model to be used for generating responses.
    pub model: String,

    // The context size of the model.
    pub context_max_tokens: u16,

    // Minimum context size for the assistant.
    pub assistant_minimum_context_tokens: i32,

    // Stop words
    pub stop_words: Vec<String>,

    // Session database url (SQLite)
    pub session_db_url: String,

    // Session name
    pub session_name: Option<String>,
}

impl AwfulJadeConfig {
    /// Ensure Conversation and Config
    ///
    /// This function checks for an existing conversation with the specified session name.
    /// If none exists, it creates a new conversation and checks for a differing AwfulConfig,
    /// creating a new config entry if necessary.
    ///
    /// # Parameters
    /// - `session_name: &str`: The name of the conversation session
    /// - `jade_config: config::AwfulJadeConfig`: The configuration for Awful Jade
    ///
    /// # Returns
    /// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
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
                }; // Assume NewConversation is a struct for creating new conversations
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
                    // Assume AwfulConfig is a struct for creating new configs
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

/// Loads the application's configuration from a YAML file.
///
/// This function reads the file at the given path, parses it as YAML, and
/// constructs an `AwfulJadeConfig` struct from it.
///
/// # Parameters
///
/// - `file`: The path to the YAML configuration file.
///
/// # Returns
///
/// - `Ok(AwfulJadeConfig)`: The loaded configuration.
/// - `Err(Box<dyn Error>)`: An error occurred while reading the file or parsing the YAML.
///
/// # Examples
///
/// ```no_run
/// use awful_aj::config::load_config;
///
/// let config_file_path = "/path/to/config.yaml";
/// match load_config(config_file_path) {
///     Ok(config) => println!("{:?}", config),
///     Err(err) => eprintln!("Error loading config: {}", err),
/// }
/// ```
pub fn load_config(file: &str) -> Result<AwfulJadeConfig, Box<dyn Error>> {
    println!("LOADING: {:?}", file);
    let content = fs::read_to_string(file)?;
    let config: AwfulJadeConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

pub fn establish_connection(db_url: &str) -> SqliteConnection {
    SqliteConnection::establish(db_url).unwrap_or_else(|_| panic!("Error connecting to {}", db_url))
}

impl PartialEq<AwfulJadeConfig> for AwfulConfig {
    fn eq(&self, other: &AwfulJadeConfig) -> bool {
        self.api_base == other.api_base
            && self.api_key == other.api_key
            && self.model == other.model
            && self.context_max_tokens as u16 == other.context_max_tokens
            && self.assistant_minimum_context_tokens == other.assistant_minimum_context_tokens
    }
}

#[cfg(test)]
mod tests {
    use crate::config_dir;

    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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

    #[test]
    fn test_load_config_invalid_file() {
        // Try to load a configuration from a non-existent file path.
        let config = load_config("non/existent/path");

        // Assert that an error occurred.
        assert!(config.is_err());
    }

    #[test]
    fn test_load_config_invalid_format() {
        // Create a temporary file with an invalid configuration format.
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, r#"invalid: config: format"#).unwrap();

        // Try to load the configuration from the temporary file.
        let config = load_config(temp_file.path().to_str().unwrap());

        // Assert that an error occurred due to the invalid format.
        assert!(config.is_err());
    }
}
