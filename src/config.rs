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

use serde::{Deserialize, Serialize};
use std::{error::Error, fs};

/// Represents the application's configuration.
///
/// This struct holds the configuration parameters needed to run the application,
/// such as API key, API base URL, and model name. It can be constructed by loading
/// a YAML configuration file using the `load_config` function.
#[derive(Serialize, Deserialize, Debug)]
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
    pub assistant_minimum_context_tokens: u16,

    // Stop words
    pub stop_words: Vec<String>,
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
    let content = fs::read_to_string(file)?;
    let config: AwfulJadeConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_valid_file() {
        // Create a temporary file with a valid configuration.
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
api_key: "example_api_key"
api_base: "http://example.com"
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
        assert_eq!(config.model, "example_model");
        assert_eq!(config.context_max_tokens, 8192u16);
        assert_eq!(config.assistant_minimum_context_tokens, 2048u16);
    }

    #[test]
    fn test_load_config_invalid_file() {
        // Try to load a configuration from a non-existent file path.
        let config = load_config("/non/existent/path");

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
