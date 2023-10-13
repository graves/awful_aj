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
use crate::{
    config::AwfulJadeConfig,
    template::{self, ChatTemplate},
    vector_store::VectorStore,
    brain::{self, Brain, Memory},
};
use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, Role},
    Client,
};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
    ExecutableCommand,
};
use futures::StreamExt;
use std::{
    error::Error,
    io::{stdout, Write},
    thread,
    time::Duration,
};
use tiktoken_rs::async_openai::get_chat_completion_max_tokens;
use tracing::{debug, error};

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
    template: ChatTemplate,
) -> Result<Vec<ChatCompletionRequestMessage>, Box<dyn Error>> {
    let mut messages = vec![ChatCompletionRequestMessage {
        role: Role::System,
        content: Some(template.system_prompt.clone()),
        name: None,
        function_call: None,
    }];
    messages.extend(template.messages);

    debug!("Prepared messages: {:?}", messages);

    Ok(messages)
}

/// Streams the response from the OpenAI API and prints it to the console in bold blue text.
///
/// This function also ensures that the assistant has a minimum number of tokens to generate a response
/// by ejecting older messages if necessary. The system message is never ejected.
///
/// # Arguments
///
/// * `client` - A reference to the OpenAI client.
/// * `model` - A string containing the model name.
/// * `messages` - A mutable vector of messages for the chat completion request.
///                This vector may be modified to ensure the assistant has enough tokens to generate a response.
/// * `config` - A reference to the configuration containing various settings including token limits.
///
/// # Returns
///
/// A Result containing a new chat completion request message to add to the conversation if successful,
/// otherwise returns an Error.
///
/// # Errors
///
/// Returns an Error if there is a problem streaming the response or handling the output.
async fn stream_response(
    client: &Client<OpenAIConfig>,
    model: String,
    mut messages: Vec<ChatCompletionRequestMessage>,
    config: &AwfulJadeConfig,
    mut vector_store: Option<&mut VectorStore>,
    brain: Option<&mut Brain>,
) -> Result<ChatCompletionRequestMessage, Box<dyn Error>> {
    let mut max_tokens = get_chat_completion_max_tokens("gpt-4", &messages)? as u16;
    debug!("Max tokens: {}", max_tokens);
    let assistant_minimum_context_tokens = std::cmp::min(
        config.assistant_minimum_context_tokens,
        config.context_max_tokens,
    );
    debug!(
        "Assistant minimum context tokens: {}",
        assistant_minimum_context_tokens
    );

    while max_tokens < assistant_minimum_context_tokens {
        if messages.len() > 1 {
            let ejected_user_message = messages.remove(1);
            let ejected_assistant_message = messages.remove(1);

            if let Some(the_vector_store) = vector_store.as_deref_mut() {
                // Corrected this line
                if let Some(content) = ejected_user_message.content {
                    let vector = the_vector_store.embed_text_to_vector(&content)?;
                    the_vector_store.add_vector_with_content(vector, content.clone())?;
                }
                if let Some(content) = ejected_assistant_message.content {
                    let vector = the_vector_store.embed_text_to_vector(&content)?;
                    the_vector_store.add_vector_with_content(vector, content.clone())?;
                }

                the_vector_store.build()?;
            }

            max_tokens = get_chat_completion_max_tokens("gpt-4", &messages)? as u16;
        } else {
            break;
        }
    }

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(max_tokens)
        .model(model)
        .messages(messages)
        .build()?;

    debug!("Sending request: {:?}", request);

    let mut response_string = String::new();

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
                        response_string.push_str(content);
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

    Ok(ChatCompletionRequestMessage {
        role: Role::Assistant,
        content: Some(response_string.clone()),
        name: None,
        function_call: None,
    })
}

/// Asks a question using the OpenAI API and prints the response.
///
/// This function handles the entire process of asking a question via the OpenAI API, including creating the client,
/// preparing messages, streaming the response, and handling errors.
///
/// # Parameters
///
/// - `config`: The configuration containing the API key, base URL, and model name.
/// - `question`: The question to be asked.
/// - `template`: The chat template containing the system prompt and initial messages.
///
/// # Returns
///
/// A result indicating the success or failure of the operation.
pub async fn ask(
    config: &AwfulJadeConfig,
    question: String,
    template: ChatTemplate,
) -> Result<(), Box<dyn Error>> {
    let client = create_client(config)?;
    let mut messages = prepare_messages(template).await?;
    messages.push(ChatCompletionRequestMessage {
        role: Role::User,
        content: Some(question.to_string()),
        name: None,
        function_call: None,
    });
    let _response = stream_response(&client, config.model.clone(), messages, &config, None, None).await?;

    Ok(())
}

/// Handles the interactive mode where the user can continuously ask questions and receive responses.
///
/// This function facilitates an interactive conversation with the OpenAI API. It uses a loop to allow the user
/// to ask multiple questions and receive responses until the user decides to exit.
///
/// # Parameters
///
/// - `config`: The configuration containing the API key, base URL, and model name.
/// - `conversation_name`: The name of the conversation.
/// - `vector_store`: The vector store for managing and storing vectors.
///
/// # Returns
///
/// A result indicating the success or failure of the operation.
pub async fn interactive_mode(
    config: &AwfulJadeConfig,
    conversation_name: String,
    mut vector_store: VectorStore,
    mut brain: Brain,
) -> Result<(), Box<dyn Error>> {
    // Display existing conversation history, or start a new conversation
    println!("Conversation: {}", conversation_name);

    // Load the default template
    let template = template::load_template("default").await?;

    // Prepare messages for API request
    let mut messages = prepare_messages(template.clone()).await?;

    loop {
        // Save the current cursor position
        let mut stdout = stdout();

        // Print "You: " with animation
        for c in "\nYou:".chars() {
            stdout.execute(Print(c))?;
            stdout.flush()?;
            thread::sleep(Duration::from_millis(100)); // Adjust the delay as needed
        }

         // Correct the cursor position after "You:"
        let (x, y) = crossterm::cursor::position()?;
        let new_x = x + " ".len() as u16; // Calculate the new x position
        stdout.execute(MoveTo(new_x, y))?; // Move the cursor to the new position

        stdout.execute(SetForegroundColor(Color::Green))?;

        stdout.flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        input = input.trim().to_string();

        stdout.execute(SetForegroundColor(Color::Reset))?;

        // Exit the loop if the user types "exit"
        if input.to_lowercase() == "exit" {
            break;
        }

        // Embed the user's input
        let vector = vector_store.embed_text_to_vector(&input)?;

        // Query the VectorStore to get relevant content based on user's input
        let neighbors = vector_store.search(&vector, 5)?;  // Adjust the number of neighbors as needed
        for neighbor_id in neighbors {
            // Here, retrieve the actual content corresponding to neighbor_id and add it to Brain's memory
            // This requires a mechanism to map IDs to actual content, which needs to be implemented in the VectorStore or another appropriate place
            let neighbor_content = "";  // Placeholder, replace with actual content retrieval
            brain.add_memory(Memory::new(neighbor_content.to_string()));
        }

        messages.push(ChatCompletionRequestMessage {
            role: Role::User,
            content: Some(input.to_string()),
            name: None,
            function_call: None,
        });

        // Get the AI's response using the OpenAI API
        let response = match stream_response(
            &create_client(config)?,
            config.model.clone(),
            messages.clone(),
            &config,
            Some(&mut vector_store),
            Some(&mut brain),
        )
        .await
        {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Error: {}", e);
                continue; // This will skip the current iteration of the loop and proceed to the next one
            }
        };
        
        messages.push(response);
    }

    Ok(())
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
            context_max_tokens: 8192,
            assistant_minimum_context_tokens: 2048,
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
        let template = mock_template();
        let messages = prepare_messages(template).await;
        assert!(messages.is_ok(), "Failed to prepare messages");
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 2, "Unexpected number of messages");
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
            context_max_tokens: 8192,
            assistant_minimum_context_tokens: 2048,
        };
        let question = "How do I write tests in Rust?".to_string();
        let template = mock_template();

        // Note: This test will fail unless you have a mock or actual API set up to handle the request
        let result = ask(&config, question, template).await;
        assert!(result.is_ok(), "Failed to ask question: {:?}", result.err());
    }

    // Add more specific test cases to handle different scenarios and edge cases
}
