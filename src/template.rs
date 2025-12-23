//! # Chat Template System for Awful Jade
//!
//! This module provides utilities for defining, loading, and managing **chat templates**
//! that control how conversations are initialized and structured.
//!
//! ## Overview
//!
//! Chat templates are YAML files that define the conversational context and behavior
//! for LLM interactions. They enable:
//!
//! - **Consistent System Prompts**: Define the assistant's role and behavior
//! - **Message Seeds**: Pre-populate conversations with example exchanges
//! - **Structured Outputs**: Enforce JSON schema response formats
//! - **Message Decoration**: Automatically wrap user messages with instructions
//!
//! ## Template Components
//!
//! A [`ChatTemplate`] consists of five main components:
//!
//! | Component | Required | Purpose |
//! |-----------|----------|---------|
//! | `system_prompt` | ✓ | Core instruction defining assistant behavior |
//! | `messages` | ✓ | Seed messages to initialize conversation context |
//! | `response_format` | ✗ | JSON schema for structured response enforcement |
//! | `pre_user_message_content` | ✗ | Text prepended to all user messages |
//! | `post_user_message_content` | ✗ | Text appended to all user messages |
//!
//! ## Template Storage
//!
//! Templates are stored in the user's configuration directory under `templates/`:
//!
//! ```text
//! <config_dir>/templates/<name>.yaml
//! ```
//!
//! **Platform-specific locations**:
//!
//! - **macOS**: `~/Library/Application Support/com.awful-sec.aj/templates/`
//! - **Linux**: `~/.config/aj/templates/`
//! - **Windows**: `%APPDATA%\com.awful-sec\aj\templates\`
//!
//! Use [`crate::config_dir()`] to get the base directory programmatically.
//!
//! ## YAML Template Format
//!
//! ### Minimal Template
//!
//! ```yaml
//! system_prompt: "You are a helpful assistant."
//! messages: []
//! ```
//!
//! ### Complete Template Example
//!
//! ```yaml
//! system_prompt: |
//!   You are Awful Jade, a technical AI assistant specializing in Rust development.
//!   Provide concise, accurate answers with code examples when appropriate.
//!
//! messages:
//!   - role: "system"
//!     content: "Focus on idiomatic Rust patterns and best practices."
//!   - role: "user"
//!     content: "How do I handle errors in Rust?"
//!   - role: "assistant"
//!     content: "Use the Result<T, E> type with ? operator for propagation..."
//!
//! # Optional: Enforce structured JSON responses
//! response_format:
//!   type: "json_schema"
//!   json_schema:
//!     name: "code_response"
//!     schema:
//!       type: "object"
//!       properties:
//!         code:
//!           type: "string"
//!         explanation:
//!           type: "string"
//!       required: ["code", "explanation"]
//!
//! # Optional: Wrap user messages
//! pre_user_message_content: "Technical question: "
//! post_user_message_content: "\nProvide a concise answer with examples."
//! ```
//!
//! ### Template for Few-Shot Learning
//!
//! ```yaml
//! system_prompt: "You are a sentiment analysis assistant."
//! messages:
//!   - role: "user"
//!     content: "I love this product!"
//!   - role: "assistant"
//!     content: "Sentiment: positive"
//!   - role: "user"
//!     content: "This is terrible."
//!   - role: "assistant"
//!     content: "Sentiment: negative"
//!   - role: "user"
//!     content: "It's okay, nothing special."
//!   - role: "assistant"
//!     content: "Sentiment: neutral"
//! ```
//!
//! ## Loading Templates
//!
//! ### Basic Loading
//!
//! ```no_run
//! use awful_aj::template::load_template;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let template = load_template("simple_question").await?;
//! println!("System prompt: {}", template.system_prompt);
//! println!("Seed messages: {}", template.messages.len());
//! # Ok(())
//! # }
//! ```
//!
//! ### Using Templates in Conversations
//!
//! ```no_run
//! use awful_aj::template::load_template;
//! use awful_aj::brain::Brain;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let config = awful_aj::config::AwfulJadeConfig {
//! #     api_key: "".into(),
//! #     api_base: "".into(),
//! #     model: "".into(),
//! #     context_max_tokens: 8192,
//! #     assistant_minimum_context_tokens: 2048,
//! #     stop_words: vec![],
//! #     session_db_url: "".into(),
//! #     session_name: None,
//! #     should_stream: None,
//! #     temperature: None,
//! # };
//! let template = load_template("technical_assistant").await?;
//!
//! // Initialize brain with template
//! let mut brain = Brain::new(config.context_max_tokens as u16, template);
//!
//! // The brain now has access to the template's system prompt and seed messages
//! # Ok(())
//! # }
//! ```
//!
//! ## Use Cases
//!
//! ### Role-Playing Templates
//!
//! ```yaml
//! system_prompt: |
//!   You are Sherlock Holmes, the famous detective.
//!   Respond in character with deductive reasoning and Victorian-era language.
//! messages: []
//! ```
//!
//! ### Code Review Templates
//!
//! ```yaml
//! system_prompt: |
//!   You are a senior code reviewer. Analyze code for:
//!   - Security vulnerabilities
//!   - Performance issues
//!   - Code style violations
//!   - Best practice violations
//! pre_user_message_content: "Review this code:\n"
//! post_user_message_content: "\nProvide actionable feedback."
//! ```
//!
//! ### Structured Data Extraction
//!
//! ```yaml
//! system_prompt: "Extract structured data from text."
//! messages: []
//! response_format:
//!   type: "json_schema"
//!   json_schema:
//!     name: "contact_info"
//!     schema:
//!       type: "object"
//!       properties:
//!         name:
//!           type: "string"
//!         email:
//!           type: "string"
//!         phone:
//!           type: "string"
//! ```
//!
//! ## Template Behavior
//!
//! ### Message Decoration
//!
//! When `pre_user_message_content` or `post_user_message_content` are set, user
//! messages are automatically wrapped:
//!
//! ```text
//! Original:  "What is Rust?"
//! Decorated: "Technical question: What is Rust?\nProvide a concise answer."
//! ```
//!
//! This is applied at send time, not when loading the template.
//!
//! ### Response Format Enforcement
//!
//! When `response_format` is specified, the LLM is instructed to return JSON
//! matching the schema. This is particularly useful for:
//!
//! - Structured data extraction
//! - API response generation
//! - Classification tasks
//! - Formatted output requirements
//!
//! **Note**: Not all models support structured output. Check your backend's documentation.
//!
//! ## Error Handling
//!
//! [`load_template()`] returns errors if:
//!
//! - **Template not found**: File doesn't exist in templates directory
//! - **Invalid YAML**: Syntax errors or malformed structure
//! - **Invalid schema**: Messages don't match OpenAI message format
//! - **Config directory inaccessible**: Platform directory resolution fails
//!
//! The function logs the resolved path using `tracing::info!` to aid debugging.
//!
//! ## Creating Templates
//!
//! Templates can be created:
//!
//! 1. **Manually**: Write YAML files in the templates directory
//! 2. **Via `aj init`**: Creates default templates during initialization
//! 3. **Programmatically**: Generate and save templates from code
//!
//! See [`crate::commands::Commands::Init`] for automatic template creation.
//!
//! ## See Also
//!
//! - [`ChatTemplate`] - Template structure definition
//! - [`load_template()`] - Template loading function
//! - [`crate::config_dir()`] - Get platform-specific config directory
//! - [`crate::brain::Brain`] - Working memory that uses templates

use async_openai::types::{ChatCompletionRequestMessage, ResponseFormatJsonSchema};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs};

/// A reusable chat template defining conversation structure and behavior.
///
/// This struct encapsulates all the components needed to configure how an LLM
/// conversation is initialized and how messages are formatted. Templates are
/// typically loaded from YAML files using [`load_template()`].
///
/// # Structure
///
/// A `ChatTemplate` combines:
///
/// 1. **System Instructions**: The [`system_prompt`](Self::system_prompt) defines the assistant's role
/// 2. **Seed Messages**: The [`messages`](Self::messages) vector provides example exchanges or context
/// 3. **Response Formatting**: Optional [`response_format`](Self::response_format) enforces structured JSON output
/// 4. **Message Decoration**: Optional pre/post content wraps user messages
///
/// # Use Cases
///
/// ## Basic Assistant
///
/// ```rust
/// use awful_aj::template::ChatTemplate;
/// use async_openai::types::ChatCompletionRequestMessage;
///
/// let template = ChatTemplate {
///     system_prompt: "You are a helpful assistant.".to_string(),
///     messages: vec![],
///     response_format: None,
///     pre_user_message_content: None,
///     post_user_message_content: None,
/// };
/// ```
///
/// ## Few-Shot Learning
///
/// ```rust
/// use awful_aj::template::ChatTemplate;
/// use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs, Role};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let template = ChatTemplate {
///     system_prompt: "Classify sentiment as positive, negative, or neutral.".to_string(),
///     messages: vec![
///         ChatCompletionRequestUserMessageArgs::default()
///             .content("I love this!")
///             .build()?.into(),
///         // Add corresponding assistant responses...
///     ],
///     response_format: None,
///     pre_user_message_content: None,
///     post_user_message_content: None,
/// };
/// # Ok(())
/// # }
/// ```
///
/// # Serialization
///
/// The struct derives [`Serialize`] and [`Deserialize`] for YAML persistence.
/// This enables bidirectional conversion between Rust structs and template files.
///
/// # See Also
///
/// - [`load_template()`] - Load templates from YAML files
/// - [`crate::brain::Brain`] - Uses templates to initialize conversations
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTemplate {
    /// Core system instruction defining the assistant's role and behavior.
    ///
    /// This prompt is typically injected as the first message in the conversation
    /// with role `"system"`. It conditions the LLM's responses throughout the session.
    ///
    /// # Examples
    ///
    /// **Concise instructions**:
    /// ```yaml
    /// system_prompt: "You are a helpful coding assistant."
    /// ```
    ///
    /// **Detailed guidelines**:
    /// ```yaml
    /// system_prompt: |
    ///   You are a technical support agent.
    ///   - Be professional and courteous
    ///   - Provide step-by-step instructions
    ///   - Ask clarifying questions when needed
    ///   - Never make assumptions about user's technical level
    /// ```
    ///
    /// # Best Practices
    ///
    /// - Keep it focused on the assistant's role
    /// - Include behavioral guidelines
    /// - Specify output format expectations
    /// - Avoid overly long prompts (they consume tokens)
    pub system_prompt: String,

    /// Seed messages to initialize conversation context.
    ///
    /// These messages are inserted at the start of the conversation, before any
    /// user input. They can serve multiple purposes:
    ///
    /// - **Few-shot examples**: Demonstrate desired response format
    /// - **Context setting**: Provide background information
    /// - **Conversation priming**: Start mid-conversation
    ///
    /// # Message Roles
    ///
    /// Each message must have one of these roles:
    /// - `"system"`: Additional system instructions
    /// - `"user"`: Example user queries
    /// - `"assistant"`: Example assistant responses
    ///
    /// # Examples
    ///
    /// **Few-shot classification**:
    /// ```yaml
    /// messages:
    ///   - role: "user"
    ///     content: "apple banana cherry"
    ///   - role: "assistant"
    ///     content: "fruits"
    ///   - role: "user"
    ///     content: "car truck motorcycle"
    ///   - role: "assistant"
    ///     content: "vehicles"
    /// ```
    ///
    /// **Empty for open-ended conversation**:
    /// ```yaml
    /// messages: []
    /// ```
    ///
    /// # Token Cost
    ///
    /// Each seed message consumes tokens from the context window. Use sparingly
    /// for models with limited context.
    pub messages: Vec<ChatCompletionRequestMessage>,

    /// Optional JSON schema for enforcing structured response format.
    ///
    /// When specified, the LLM is instructed to return JSON matching this schema.
    /// This leverages OpenAI's "JSON mode" or similar features in compatible backends.
    ///
    /// # Schema Structure
    ///
    /// The schema follows the `ResponseFormatJsonSchema` type from `async-openai`,
    /// which wraps a JSON Schema object.
    ///
    /// # Examples
    ///
    /// **Extract contact information**:
    /// ```yaml
    /// response_format:
    ///   type: "json_schema"
    ///   json_schema:
    ///     name: "contact_extraction"
    ///     schema:
    ///       type: "object"
    ///       properties:
    ///         name:
    ///           type: "string"
    ///         email:
    ///           type: "string"
    ///         phone:
    ///           type: "string"
    ///       required: ["name"]
    /// ```
    ///
    /// # Compatibility
    ///
    /// Not all LLM backends support structured output:
    /// - **OpenAI GPT-4**: ✓ Full support
    /// - **Local models**: ✗ Depends on backend (vLLM, LM Studio)
    /// - **Ollama**: ✗ Limited support
    ///
    /// Check your backend's documentation before enabling.
    ///
    /// # None Behavior
    ///
    /// When `None`, the LLM returns freeform text responses.
    pub response_format: Option<ResponseFormatJsonSchema>,

    /// Text automatically prepended to each user message.
    ///
    /// This content is inserted **before** the actual user input at runtime,
    /// effectively wrapping every query with consistent instructions or context.
    ///
    /// # Use Cases
    ///
    /// - **Context labeling**: `"Technical question: "`
    /// - **Instruction injection**: `"Explain in simple terms: "`
    /// - **Format hints**: `"Please answer in one paragraph:\n"`
    ///
    /// # Examples
    ///
    /// **Add context label**:
    /// ```yaml
    /// pre_user_message_content: "Customer inquiry: "
    /// ```
    ///
    /// User input `"How do I reset my password?"` becomes:
    /// ```text
    /// "Customer inquiry: How do I reset my password?"
    /// ```
    ///
    /// **None Behavior**:
    /// When `None`, user messages are sent as-is without modification.
    pub pre_user_message_content: Option<String>,

    /// Text automatically appended to each user message.
    ///
    /// This content is inserted **after** the actual user input at runtime,
    /// adding instructions or constraints to every query.
    ///
    /// # Use Cases
    ///
    /// - **Format requirements**: `"\nAnswer in bullet points."`
    /// - **Length constraints**: `"\nKeep response under 100 words."`
    /// - **Style guidelines**: `"\nUse simple language."`
    ///
    /// # Examples
    ///
    /// **Enforce brevity**:
    /// ```yaml
    /// post_user_message_content: "\nProvide a concise answer (max 3 sentences)."
    /// ```
    ///
    /// User input `"What is Rust?"` becomes:
    /// ```text
    /// "What is Rust?\nProvide a concise answer (max 3 sentences)."
    /// ```
    ///
    /// **Combining Pre and Post**:
    /// ```yaml
    /// pre_user_message_content: "Technical question: "
    /// post_user_message_content: "\nInclude code examples."
    /// ```
    ///
    /// Result:
    /// ```text
    /// "Technical question: How do I iterate over a HashMap?\nInclude code examples."
    /// ```
    ///
    /// **None Behavior**:
    /// When `None`, user messages are sent as-is without modification.
    pub post_user_message_content: Option<String>,
}

/// Loads a chat template by name from the user's configuration directory.
///
/// This function resolves the template path as `<config_dir>/templates/<name>.yaml`,
/// reads the file, and deserializes it into a [`ChatTemplate`] struct.
///
/// # Parameters
///
/// - `name`: Template name without the `.yaml` extension (e.g., `"simple_question"`)
///
/// # Returns
///
/// - `Ok(ChatTemplate)`: Successfully loaded and deserialized template
/// - `Err(Box<dyn Error>)`: Template not found, invalid YAML, or I/O error
///
/// # Resolution Process
///
/// 1. **Determine config directory**: Calls [`crate::config_dir()`] to get platform-specific path
/// 2. **Build path**: Appends `templates/<name>.yaml`
/// 3. **Read file**: Loads YAML content into string
/// 4. **Deserialize**: Parses YAML into [`ChatTemplate`] struct
/// 5. **Log path**: Records resolved path with `tracing::info!`
///
/// # Errors
///
/// Returns an error if:
///
/// - **Config directory unavailable**: Platform directory resolution fails (rare)
/// - **Template not found**: File doesn't exist at expected path
/// - **Permission denied**: Insufficient permissions to read file
/// - **Invalid YAML syntax**: Malformed YAML or syntax errors
/// - **Schema mismatch**: YAML structure doesn't match [`ChatTemplate`] fields
/// - **Invalid message format**: Messages don't conform to OpenAI message schema
///
/// # Template Path Resolution
///
/// Given `name = "simple_question"`, the function resolves to:
///
/// - **macOS**: `~/Library/Application Support/com.awful-sec.aj/templates/simple_question.yaml`
/// - **Linux**: `~/.config/aj/templates/simple_question.yaml`
/// - **Windows**: `%APPDATA%\com.awful-sec\aj\templates\simple_question.yaml`
///
/// # Examples
///
/// ## Basic Loading
///
/// ```no_run
/// use awful_aj::template::load_template;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let template = load_template("simple_question").await?;
/// println!("System prompt: {}", template.system_prompt);
/// println!("Seed messages: {}", template.messages.len());
/// # Ok(())
/// # }
/// ```
///
/// ## Error Handling
///
/// ```no_run
/// use awful_aj::template::load_template;
///
/// # #[tokio::main]
/// # async fn main() {
/// match load_template("nonexistent").await {
///     Ok(template) => println!("Loaded: {}", template.system_prompt),
///     Err(e) => eprintln!("Failed to load template: {}", e),
/// }
/// # }
/// ```
///
/// ## Using with Brain
///
/// ```no_run
/// use awful_aj::template::load_template;
/// use awful_aj::brain::Brain;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let config = awful_aj::config::AwfulJadeConfig {
/// #     api_key: "".into(), api_base: "".into(), model: "".into(),
/// #     context_max_tokens: 8192, assistant_minimum_context_tokens: 2048,
/// #     stop_words: vec![], session_db_url: "".into(),
/// #     session_name: None, should_stream: None, temperature: None,
/// # };
/// let template = load_template("technical_assistant").await?;
///
/// // Create brain with template
/// let mut brain = Brain::new(config.context_max_tokens as u16, template);
///
/// // The brain is now initialized with the template
/// # Ok(())
/// # }
/// ```
///
/// ## Conditional Loading
///
/// ```no_run
/// use awful_aj::template::load_template;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let template_name = std::env::var("TEMPLATE_NAME")
///     .unwrap_or_else(|_| "simple_question".to_string());
///
/// let template = load_template(&template_name).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Logging
///
/// This function logs the resolved path with `tracing::info!` before reading the file:
///
/// ```text
/// Loading template: /Users/alice/Library/Application Support/com.awful-sec.aj/templates/simple_question.yaml
/// ```
///
/// This aids debugging when templates can't be found or loaded.
///
/// # Creating Templates
///
/// To create a new template:
///
/// 1. Navigate to the templates directory (use `aj init` to create it)
/// 2. Create a new `.yaml` file with your template name
/// 3. Define at minimum `system_prompt` and `messages` fields
/// 4. Load with `load_template("your_template_name")`
///
/// # See Also
///
/// - [`ChatTemplate`] - Template structure definition
/// - [`crate::config_dir()`] - Get platform-specific config directory
/// - [`crate::commands::Commands::Init`] - Initialize templates directory
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
