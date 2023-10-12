//! This module handles interactions with the OpenAI API for asking questions and receiving responses.
//!
//! It provides functions to create a client, prepare messages, and stream responses from the API.
//! The responses from the OpenAI API are printed in bold blue text to the console.
//!
//! # Example
//!
//! ```no_run
//! use awful_aj::{AwfulJadeConfig, ChatTemplate, ask};
//!
//! // Load your configuration and template, and prepare your question
//! let config = AwfulJadeConfig::new(/* ... */);
//! let template = ChatTemplate::new(/* ... */);
//! let question = "What is the meaning of life?".to_string();
//!
//! // Ask a question using the OpenAI API
//! let _ = ask(&config, question, template);
//! ```

use super::config::AwfulJadeConfig;
use super::template::ChatTemplate;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, CreateChatCompletionRequestArgs,
        CreateChatCompletionResponse, Role,
    },
    Client,
};
use crossterm::{
    style::{Attribute, Color, SetAttribute, SetForegroundColor},
    ExecutableCommand,
};
use futures::StreamExt;
use std::{
    error::Error,
    io::{stdout, Write},
};
use tracing::{debug, error};

/// Asks a question using the OpenAI API and prints the response in bold blue text.
///
/// # Arguments
///
/// * `config` - A reference to the configuration object containing the API key, base URL, and model name.
/// * `question` - A `String` containing the question to be asked.
/// * `template` - A `ChatTemplate` object containing the system prompt and optional initial messages.
///
/// # Returns
///
/// A Result with an empty tuple if successful, otherwise returns an Error.
///
/// # Errors
///
/// Returns an Error if there is a problem creating the client, preparing messages, or streaming the response.
pub async fn ask(
    config: &AwfulJadeConfig,
    question: String,
    template: ChatTemplate,
) -> Result<(), Box<dyn Error>> {
    let client = create_client(config)?;
    let messages = prepare_messages(&question, template).await?;
    let _response = stream_response(&client, config.model.clone(), messages).await?;

    Ok(())
}

/// Creates a new OpenAI client using the provided configuration.
///
/// # Arguments
///
/// * `config` - A reference to the configuration object containing the API key and base URL.
///
/// # Returns
///
/// A Result containing the created client if successful, otherwise returns an Error.
///
/// # Errors
///
/// Returns an Error if there is a problem creating the client.
fn create_client(config: &AwfulJadeConfig) -> Result<Client<OpenAIConfig>, Box<dyn Error>> {
    let openai_config = OpenAIConfig::new()
        .with_api_key(config.api_key.clone())
        .with_api_base(config.api_base.clone());
    debug!("Client created with config: {:?}", openai_config);
    Ok(Client::with_config(openai_config))
}

/// Prepares a vector of messages for the chat completion request, including the system prompt and user's question.
///
/// # Arguments
///
/// * `question` - A reference to the string containing the user's question.
/// * `template` - A `ChatTemplate` object containing the system prompt and optional initial messages.
///
/// # Returns
///
/// A Result containing a vector of prepared messages if successful, otherwise returns an Error.
///
/// # Errors
///
/// Returns an Error if there is a problem preparing the messages.
async fn prepare_messages(
    question: &str,
    template: ChatTemplate,
) -> Result<Vec<ChatCompletionRequestMessage>, Box<dyn Error>> {
    let mut messages = vec![ChatCompletionRequestMessage {
        role: Role::System,
        content: Some(template.system_prompt.clone()),
        name: None,
        function_call: None,
    }];
    messages.extend(template.messages);
    messages.push(ChatCompletionRequestMessage {
        role: Role::User,
        content: Some(question.to_string()),
        name: None,
        function_call: None,
    });

    debug!("Prepared messages: {:?}", messages);

    Ok(messages)
}

/// Streams the response from the OpenAI API and prints it to the console in bold blue text.
///
/// # Arguments
///
/// * `client` - A reference to the OpenAI client.
/// * `model` - A string containing the model name.
/// * `messages` - A vector of messages for the chat completion request.
///
/// # Returns
///
/// A Result containing the chat completion response if successful, otherwise returns an Error.
///
/// # Errors
///
/// Returns an Error if there is a problem streaming the response or handling the output.
async fn stream_response(
    client: &Client<OpenAIConfig>,
    model: String,
    messages: Vec<ChatCompletionRequestMessage>,
) -> Result<CreateChatCompletionResponse, Box<dyn Error>> {
    let token_count: u16 = messages
        .iter()
        .map(|msg| msg.content.as_ref().unwrap().split_whitespace().count() as u16)
        .sum();
    let max_tokens = 2048u16.saturating_sub(token_count);

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(max_tokens)
        .model(model)
        .messages(messages)
        .build()?;

    debug!("Sending request: {:?}", request);

    let mut stream = client.chat().create_stream(request).await?;
    let mut lock = stdout().lock();
    let mut stdout = std::io::stdout();
    stdout.execute(SetForegroundColor(Color::Blue))?;
    stdout.execute(SetAttribute(Attribute::Bold))?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                debug!("Received response: {:?}", response);
                response.choices.iter().for_each(|chat_choice| {
                    if let Some(ref content) = chat_choice.delta.content {
                        write!(lock, "{}", content).unwrap();
                    }
                });
            }
            Err(err) => {
                error!("Received error: {}", err);
                writeln!(lock, "error: {}", err).unwrap();
            }
        }
        stdout.flush()?;
    }

    stdout.execute(SetAttribute(Attribute::Reset))?;
    stdout.execute(SetForegroundColor(Color::Reset))?;
    Ok(CreateChatCompletionResponse {
        id: String::new(),
        object: String::new(),
        created: 0,
        model: String::new(),
        usage: Default::default(),
        choices: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::Role;
    use httpmock::prelude::*;
    use serde_json::json;
    use tracing_subscriber;

    fn setup() {
        let _ = tracing_subscriber::fmt::try_init();
    }

    // Mock configuration for testing
    fn mock_config() -> AwfulJadeConfig {
        AwfulJadeConfig {
            api_key: "mock_api_key".to_string(),
            api_base: "http://mock.api.base".to_string(),
            model: "mock_model".to_string(),
        }
    }

    // Mock template for testing
    fn mock_template() -> ChatTemplate {
        ChatTemplate {
            system_prompt: "You are Awful Jade, a helpful AI assistant.".to_string(),
            messages: vec![ChatCompletionRequestMessage {
                role: Role::User,
                content: Some("How do I read a file in Rust?".to_string()),
                name: None,
                function_call: None,
            }],
        }
    }

    #[tokio::test]
    async fn test_create_client() {
        let config = mock_config();
        let client = create_client(&config);
        assert!(client.is_ok(), "Failed to create client");
    }

    #[tokio::test]
    async fn test_prepare_messages() {
        let question = "How do I write tests in Rust?".to_string();
        let template = mock_template();
        let messages = prepare_messages(&question, template).await;
        assert!(messages.is_ok(), "Failed to prepare messages");
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 3, "Unexpected number of messages");
    }

    #[tokio::test]
    async fn test_ask() {
        setup();
        // Start a mock server.
        let server = MockServer::start();

        // Create a mock for the chat completions endpoint.
        let _mock = server.mock(|when, then| {
        when.method(POST)
            .path("/chat/completions")
            .header("content-type", "application/json")
            .header("authorization", "Bearer mock_api_key")
            .json_body(json!({
                "model": "mock_model",
                "messages": [
                    {
                        "role": "system",
                        "content": "You are Awful Jade, a helpful AI assistant."
                    },
                    {
                        "role": "user",
                        "content": "How do I read a file in Rust?"
                    },
                    {
                        "role": "user",
                        "content": "How do I write tests in Rust?"
                    }
                ],
                "stream": true,
                "max_tokens": 2025
            }));
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": "chatcmpl-1234567890",
                "object": "chat.completion",
                "created": 0,
                "model": "text-davinci-002",
                "usage": { "prompt_tokens": 56, "completion_tokens": 31, "total_tokens": 87 },
                "choices": [
                    {
                        "message": {
                            "role": "assistant",
                            "content": "To write tests in Rust, you can use the built-in `test` module...",
                        },
                        "finish_reason": "stop",
                        "index": 0
                    }
                ]
            }));
        });

        // Now you can run your test with the mock server's URL.
        let config = AwfulJadeConfig {
            api_key: "mock_api_key".to_string(),
            api_base: server.url(""), // Use the mock server's URL
            model: "mock_model".to_string(),
        };
        let question = "How do I write tests in Rust?".to_string();
        let template = mock_template();

        // Note: This test will fail unless you have a mock or actual API set up to handle the request
        let result = ask(&config, question, template).await;
        assert!(
            result.is_ok(),
            "Failed to ask question: {:?}", 
            result.err()
        );
    }

    // Add more specific test cases to handle different scenarios and edge cases
}
