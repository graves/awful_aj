//! This module provides functionality for loading and handling chat templates.
//!
//! It defines the `ChatTemplate` struct, which holds the system prompt and messages,
//! and a `load_template` function to load the template from a file.
//!
//! # Examples
//!
//! Loading a chat template from a file:
//!
//! ```no_run
//! use awful_jade::template::{ChatTemplate, load_template};
//!
//! let template_name = "example";
//! let template: ChatTemplate = load_template(template_name).unwrap();
//! println!("{:?}", template);
//! ```

use async_openai::types::ChatCompletionRequestMessage;
use serde_derive::{Deserialize, Serialize};
use std::{error::Error, fs};
use tracing::debug;

/// Represents a chat template.
///
/// This struct holds the system prompt and a list of messages that constitute
/// a chat template. It can be constructed by loading a YAML file using the
/// `load_template` function.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatTemplate {
    /// The system prompt that guides the assistant's behavior.
    pub system_prompt: String,

    /// A list of messages that are part of the chat template.
    pub messages: Vec<ChatCompletionRequestMessage>,
}

/// Loads a chat template from a YAML file.
///
/// This function reads the file at the given path, parses it as YAML, and
/// constructs a `ChatTemplate` struct from it.
///
/// # Parameters
///
/// - `name`: The name of the YAML file (without extension) that contains the chat template.
///
/// # Returns
///
/// - `Ok(ChatTemplate)`: The loaded chat template.
/// - `Err(Box<dyn Error>)`: An error occurred while reading the file or parsing the YAML.
///
/// # Examples
///
/// ```no_run
/// use awful_aj::template::load_template;
///
/// let template_name = "example";
/// match load_template(template_name) {
///     Ok(template) => println!("{:?}", template),
///     Err(err) => eprintln!("Error loading template: {}", err),
/// }
/// ```
pub async fn load_template(name: &str) -> Result<ChatTemplate, Box<dyn Error>> {
    let path = format!("templates/{}.yaml", name);
    let config_dir = crate::config_dir()?.join(&path);

    debug!("Loading template: {}", config_dir.display());

    let content = fs::read_to_string(path)?;
    let template: ChatTemplate = serde_yaml::from_str(&content)?;

    Ok(template)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::Path};
    use tempfile::NamedTempFile;
    use tokio;

    #[tokio::test]
    async fn test_load_template_valid_file() {
        // Ensure the templates directory exists
        let templates_dir = Path::new("templates");
        if !templates_dir.exists() {
            fs::create_dir(templates_dir).expect("Failed to create templates directory");
        }

        // Create a temporary file within the templates directory
        let file_content = r#"
    system_prompt: "You are a helpful assistant."
    messages:
      - role: "user"
        content: "What is the weather like?"
    "#;

        let file_name = "valid_template";
        let file_path = templates_dir.join(format!("{}.yaml", file_name));
        fs::write(&file_path, file_content).expect("Unable to write to temporary file");

        // Attempt to load the template
        let template = load_template(file_name).await;

        // Clean up the temporary file
        fs::remove_file(file_path).expect("Unable to delete temporary file");

        assert!(template.is_ok(), "Failed to load valid template");
    }

    #[tokio::test]
    async fn test_load_template_invalid_file() {
        // Try to load a template from a non-existent file path.
        let template = load_template("/non/existent/path").await;

        // Assert that an error occurred.
        assert!(template.is_err());
    }

    #[tokio::test]
    async fn test_load_template_invalid_format() {
        // Create a temporary file with an invalid template format.
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, r#"invalid: template: format"#).unwrap();

        // Try to load the template from the temporary file.
        let template = load_template(temp_file.path().to_str().unwrap()).await;

        // Assert that an error occurred due to the invalid format.
        assert!(template.is_err());
    }
}
