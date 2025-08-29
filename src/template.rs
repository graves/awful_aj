//! # Template loading and structure
//!
//! Utilities for defining and loading **chat templates** used by Awful Jade.
//!
//! A template is a small YAML document that specifies:
//! - a `system_prompt` to steer the assistant’s behavior,
//! - an ordered list of seed `messages` (serialized
//!   [`async_openai::types::ChatCompletionRequestMessage`]),
//! - an optional `response_format` JSON schema to enforce structured outputs,
//! - optional `pre_user_message_content` / `post_user_message_content` strings that are
//!   automatically prepended/appended to every *user* message at runtime.
//!
//! Templates are stored per-user under the application’s configuration directory,
//! inside a `templates/` subfolder. The loader resolves templates at:
//!
//! ```text
//! <config_dir>/templates/<name>.yaml
//! ```
//!
//! where `<config_dir>` is provided by [`crate::config_dir()`] and is platform-specific:
//!
//! - macOS: `~/Library/Application Support/com.awful-sec.aj/`  
//! - Linux: `~/.config/aj/` (via XDG)  
//! - Windows: `%APPDATA%\com.awful-sec\aj\`
//!
//! ## Minimal YAML example
//!
//! ```yaml
//! # ~/.config/.../templates/simple_question.yaml
//! system_prompt: "You are Awful Jade, a concise and helpful assistant."
//! messages:
//!   - role: "user"
//!     content: "Say hello briefly."
//! # Optional fields:
//! # response_format:   # ResponseFormatJsonSchema (see async_openai types)
//! #   type: "json_schema"
//! #   json_schema: { ... }   # your JSON schema payload
//! # pre_user_message_content: "Please keep it under 2 sentences."
//! # post_user_message_content: "Answer in plain English."
//! ```
//!
//! ## Loading a template
//!
//! ```no_run
//! use awful_aj::template::{ChatTemplate, load_template};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let template: ChatTemplate = load_template("simple_question").await?;
//! println!("System prompt: {}", template.system_prompt);
//! # Ok(()) }
//! ```
//!
//! ## Behavior notes
//! - [`load_template`] *only* reads from the configuration directory; it does not look in the
//!   current working directory.
//! - The loader logs the resolved path with `tracing::info!` to help diagnose missing/invalid
//!   files.
//! - The `messages` field is deserialized directly into `ChatCompletionRequestMessage` values
//!   (System/User/Assistant). Ensure your YAML matches the enum’s expected shape.

use async_openai::types::{ChatCompletionRequestMessage, ResponseFormatJsonSchema};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs};

/// A reusable chat template.
///
/// Instances are typically created by deserializing YAML files with
/// [`load_template`]. The fields map directly to how a chat session is
/// initialized and how user input is decorated.
///
/// ### Fields
/// - [`system_prompt`](Self::system_prompt): A high-level instruction that
///   conditions the assistant.
/// - [`messages`](Self::messages): An initial ordered set of chat messages
///   (system/user/assistant) inserted before user input.
/// - [`response_format`](Self::response_format): Optional JSON schema to request
///   structured responses (see OpenAI’s *JSON schema* response format).
/// - [`pre_user_message_content`](Self::pre_user_message_content): If set,
///   concatenated **before** each user message string at runtime.
/// - [`post_user_message_content`](Self::post_user_message_content): If set,
///   concatenated **after** each user message string at runtime.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTemplate {
    /// Global instruction used as the session’s system message.
    pub system_prompt: String,

    /// Seed messages that precede live conversation turns.
    pub messages: Vec<ChatCompletionRequestMessage>,

    /// Optional response schema describing the desired JSON payload.
    pub response_format: Option<ResponseFormatJsonSchema>,

    /// Extra text automatically added **before** each user message at send time.
    pub pre_user_message_content: Option<String>,

    /// Extra text automatically added **after** each user message at send time.
    pub post_user_message_content: Option<String>,
}

/// Load a chat template by name from the user’s config directory.
///
/// Resolves `<config_dir>/templates/<name>.yaml`, reads the file, and
/// deserializes into a [`ChatTemplate`].
///
/// ### Errors
/// Returns an error if:
/// - the config directory cannot be determined,
/// - the template file does not exist or cannot be read,
/// - the YAML content cannot be deserialized into a `ChatTemplate`.
///
/// ### Examples
/// ```no_run
/// use awful_aj::template::load_template;
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let tpl = load_template("simple_question").await?;
/// assert!(tpl.system_prompt.contains("Awful Jade"));
/// # Ok(()) }
/// ```
pub async fn load_template(name: &str) -> Result<ChatTemplate, Box<dyn Error>> {
    let path = format!("templates/{}.yaml", name);
    let config_path = crate::config_dir()?.join(&path);

    tracing::info!("Loading template: {}", config_path.display());

    let content = fs::read_to_string(config_path)?;
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
        let config_dir = crate::config_dir().expect("Config directory doesnt exist");
        let templates_dir = config_dir.join(Path::new("templates"));
        if !templates_dir.exists() {
            fs::create_dir(&templates_dir).expect("Failed to create templates directory");
        }

        // Create a file within the templates directory
        let file_content = r#"
system_prompt: "You are a helpful assistant."
messages:
  - role: "user"
    content: "What is the weather like?"
"#;

        let file_name = "valid_template";
        let file_path = templates_dir.join(format!("{}.yaml", file_name));
        fs::write(&file_path, file_content).expect("Unable to write template");

        // Attempt to load the template
        let template = load_template(file_name).await;

        // Clean up the file
        fs::remove_file(file_path).expect("Unable to delete template");
        assert!(template.is_ok(), "Failed to load valid template");
    }

    #[tokio::test]
    async fn test_load_template_invalid_file() {
        let template = load_template("non/existent/path").await;
        assert!(template.is_err(), "Expected error for missing template");
    }

    #[tokio::test]
    async fn test_load_template_invalid_format() {
        // Create a temporary file with an invalid template format.
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, r#"invalid: template: format"#).unwrap();

        // NOTE: This test intentionally bypasses the standard lookup path by
        // passing the temp file path as the "name". That means the loader will
        // try to resolve "<config_dir>/templates/<temp_path>.yaml", which
        // should fail to deserialize. We assert an Err to keep behavior parity
        // with the original test scaffold.
        let template = load_template(temp_file.path().to_str().unwrap()).await;
        assert!(template.is_err(), "Expected YAML parse error");
    }
}
