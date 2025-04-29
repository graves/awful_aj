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
    brain::{Brain, Memory},
    config::{establish_connection, AwfulJadeConfig},
    session_messages::SessionMessages,
    template::ChatTemplate,
    vector_store::VectorStore,
};
use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, Role},
    Client,
};
use crossterm::{
    cursor::MoveTo,
    style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
    ExecutableCommand,
};
use futures::StreamExt;
use hora::core::{ann_index::ANNIndex, node::Node};
use std::{
    error::Error,
    io::{stdout, Write},
    thread,
    time::Duration,
};

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
async fn stream_response<'a>(
    client: &Client<OpenAIConfig>,
    model: String,
    session_messages: &mut SessionMessages,
    config: &AwfulJadeConfig,
    mut vector_store: Option<&mut VectorStore>,
    _brain: Option<&mut Brain<'a>>,
) -> Result<ChatCompletionRequestMessage, Box<dyn Error>> {
    while session_messages.should_eject_message() {
        if session_messages.conversation_messages.len() > 0 {
            let ejected_user_message = session_messages.conversation_messages.remove(0);
            let ejected_assistant_message = session_messages.conversation_messages.remove(0);

            if let Some(the_vector_store) = vector_store.as_deref_mut() {
                if let Some(content) = ejected_user_message.content {
                    let vector = the_vector_store.embed_text_to_vector(&content)?;
                    let memory = Memory::new(Role::User, content.clone());
                    let res = the_vector_store.add_vector_with_content(vector, memory);
                    if !res.is_err() {
                        the_vector_store.build()?;
                    }
                }
                if let Some(content) = ejected_assistant_message.content {
                    let vector = the_vector_store.embed_text_to_vector(&content)?;
                    let memory = Memory::new(Role::Assistant, content.clone());
                    let res = the_vector_store.add_vector_with_content(vector, memory);
                    if !res.is_err() {
                        the_vector_store.build()?;
                    }
                }
            }
        } else {
            break;
        }
    }

    let mut full_conversation = Vec::new();
    full_conversation.append(&mut session_messages.preamble_messages);
    full_conversation.append(&mut session_messages.conversation_messages);

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(config.context_max_tokens)
        .model(model)
        .stop(config.stop_words.clone())
        .messages(full_conversation)
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

    drop(lock);

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
pub async fn ask<'a>(
    config: &AwfulJadeConfig,
    question: String,
    template: &ChatTemplate,
    vector_store: Option<&mut VectorStore>,
    mut brain: Option<&mut Brain<'a>>,
) -> Result<(), Box<dyn Error>> {
    let client = create_client(config)?;
    let mut session_messages = get_session_messages(&brain, config, template, &question).unwrap();
    let _added_memories_to_brain_result =
        add_memories_to_brain(&vector_store, &question, &mut session_messages, &mut brain);

    let _convo_messages_insertion_result =
        session_messages
            .conversation_messages
            .push(ChatCompletionRequestMessage {
                role: Role::User,
                content: Some(question.to_string()),
                name: None,
                function_call: None,
            });

    let assistant_response = stream_response(
        &client,
        config.model.clone(),
        &mut session_messages,
        &config,
        vector_store,
        brain,
    )
    .await?;

    let _diesel_sqlite_response = session_messages
        .insert_message("assistant".to_string(), assistant_response.content.unwrap());

    Ok(())
}

fn get_session_messages(
    brain: &Option<&mut Brain>,
    config: &AwfulJadeConfig,
    mut template: &ChatTemplate,
    question: &String,
) -> Result<SessionMessages, Box<dyn Error>> {
    let session_messages = if config.session_name.is_some() && brain.is_some() {
        let prepare_brain = brain.as_ref().expect("Brain not found!");
        let session_messages =
            prepare_messages_for_existing_session(&mut template, config, prepare_brain)?;

        let mut connection = establish_connection(&config.session_db_url);

        let a_session_name = config
            .session_name
            .as_ref()
            .expect("No session name on AwfulJadeConfig");
        let conversation: Result<Conversation, diesel::result::Error> =
            connection.transaction(|conn| {
                let existing_conversation: Result<Conversation, diesel::result::Error> =
                    crate::schema::conversations::table
                        .filter(crate::schema::conversations::session_name.eq(a_session_name))
                        .first(conn);

                existing_conversation
            });

        let _res: Message = connection.transaction(|conn| {
            let serialized_message = Message {
                id: None,
                role: "user".to_string(),
                content: question.to_string(),
                dynamic: false,
                conversation_id: Some(conversation.expect("Conversation doesnt exist").id.unwrap()),
            };
            diesel::insert_into(crate::schema::messages::table)
                .values(&serialized_message)
                .returning(Message::as_returning())
                .get_result(conn)
        })?;

        session_messages
    } else {
        let prepare_brain = brain.as_ref().expect("Brain not found!");
        let session_messages = prepare_messages(&mut template, &config, prepare_brain).unwrap();

        session_messages
    };

    Ok(session_messages)
}

fn add_memories_to_brain(
    vector_store: &Option<&mut VectorStore>,
    question: &String,
    session_messages: &mut SessionMessages,
    brain: &mut Option<&mut Brain>,
) -> Result<(), Box<dyn Error>> {
    if let Some(ref vector_store) = vector_store {
        // Embed the user's input
        let vector = vector_store.embed_text_to_vector(&question)?;

        // Query the VectorStore to get relevant content based on user's input
        let neighbor_vectors = vector_store.index.search_nodes(&vector, 3); // Adjust the number of neighbors as needed

        let neighbor_vec_distances = neighbor_vectors.iter().map(|v| {
            let (node, distance): (Node<f32, usize>, f32) = v.clone();
            (node.vectors().clone(), node.idx().clone(), distance)
        });

        for (_vector, id, euclidean_distance) in neighbor_vec_distances {
            // Here, retrieve the actual content corresponding to neighbor_id and add it to Brain's memory
            // This requires a mechanism to map IDs to actual content, which needs to be implemented in the VectorStore or another appropriate place
            if let Some(neighbor_content) = vector_store.get_content_by_id(id.unwrap()) {
                if let Some(ref mut brain) = brain {
                    if euclidean_distance < 1.0 {
                        brain.add_memory((*neighbor_content).clone(), session_messages);
                    }
                }
            }
        }

        if let Some(ref mut brain) = brain {
            session_messages.preamble_messages = brain.build_preamble().unwrap();
        }
    }

    Ok(())
}

fn prepare_messages(
    template: &ChatTemplate,
    config: &AwfulJadeConfig,
    brain: &Brain,
) -> Result<SessionMessages, Box<dyn Error>> {
    let mut session_messages = SessionMessages::new(config.clone());
    let mut preamble_messages = brain.build_preamble().unwrap();
    let mut template_messages = template.messages.clone();

    session_messages
        .preamble_messages
        .append(&mut preamble_messages);
    session_messages
        .preamble_messages
        .append(&mut template_messages);

    Ok(session_messages)
}

use crate::models::*;
use diesel::prelude::*;
fn prepare_messages_for_existing_session(
    template: &ChatTemplate,
    config: &AwfulJadeConfig,
    brain: &Brain,
) -> Result<SessionMessages, Box<dyn Error>> {
    let mut session_messages = SessionMessages::new(config.clone());

    let conversation: Result<Conversation, diesel::result::Error> =
        session_messages.query_conversation();

    match conversation {
        Ok(conversation) => {
            let recent_messages = session_messages.query_conversation_messages(&conversation);

            if let Ok(mut recent_msgs) = recent_messages {
                // If there are recent_msgs then the first 3 are System Prompt, Brain Message, Assistant Acknowledgement, the N Template Messages
                if recent_msgs.len() > 0 {
                    let preamble_messages = recent_msgs.drain(0..(3 + template.messages.len()));
                    for msg in preamble_messages {
                        let role = SessionMessages::string_to_role(&msg.role);

                        let msg_obj = ChatCompletionRequestMessage {
                            role: role,
                            content: Some(msg.content.clone()),
                            name: None,
                            function_call: None,
                        };

                        session_messages.preamble_messages.push(msg_obj);
                    }

                    for msg in recent_msgs.into_iter() {
                        let role = SessionMessages::string_to_role(&msg.role);

                        let chat_completion_message =
                            SessionMessages::serialize_chat_completion_message(
                                role,
                                msg.clone().content,
                            );

                        session_messages
                            .conversation_messages
                            .push(chat_completion_message);
                    }
                } else {
                    let mut preamble_messages =
                        brain.build_preamble().expect("Failed to build preamble");

                    let _res =
                        session_messages.persist_chat_completion_messages(&preamble_messages);
                    session_messages
                        .preamble_messages
                        .append(&mut preamble_messages);

                    let template_messages = template.messages.clone();

                    for message in template_messages {
                        let msg_clone = message.clone();
                        let serialized_message = Message {
                            id: None,
                            role: message.role.to_string(),
                            content: message.content.expect("Message content empty"),
                            dynamic: false,
                            conversation_id: conversation.id,
                        };

                        let _res = session_messages.persist_message(&serialized_message);

                        session_messages.conversation_messages.push(msg_clone);
                    }
                }
            }

            Ok(session_messages)
        }
        Err(_) => prepare_messages(template, config, brain),
    }
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
pub async fn interactive_mode<'a>(
    config: &AwfulJadeConfig,
    mut vector_store: VectorStore,
    mut brain: Brain<'a>,
    template: &ChatTemplate,
) -> Result<(), Box<dyn Error>> {
    // Display existing conversation history, or start a new conversation
    println!("Conversation: {}", config.session_name.clone().unwrap());

    let client = create_client(config)?;

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

        let mut session_messages =
            get_session_messages(&Some(&mut brain), config, template, &input).unwrap();
        let _added_memories_to_brain_result =
            add_memories_to_brain(&Some(&mut vector_store), &input, &mut session_messages, &mut Some(&mut brain));

        let _convo_messages_insertion_result =
            session_messages
                .conversation_messages
                .push(ChatCompletionRequestMessage {
                    role: Role::User,
                    content: Some(input.to_string()),
                    name: None,
                    function_call: None,
                });

        // Get the AI's response using the OpenAI API
        let assistant_response = match stream_response(
            &client,
            config.model.clone(),
            &mut session_messages,
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

        session_messages.conversation_messages.push(assistant_response.clone());

        let _diesel_sqlite_response = session_messages
        .insert_message("assistant".to_string(), assistant_response.content.unwrap());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::brain;

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
            stop_words: vec!["\n<|im_start|>".to_string(), "<|im_end|>".to_string()],
            session_db_url: "/Users/tg/Projects/awful_aj/test.db".to_string(),
            session_name: None,
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
        let brain = Brain::new(8092, &template);
        let config = AwfulJadeConfig {
            api_key: "".to_string(),
            api_base: "".to_string(),
            model: "".to_string(),
            context_max_tokens: 8092,
            assistant_minimum_context_tokens: 2048,
            stop_words: vec!["".to_string()],
            session_db_url: "".to_string(),
            session_name: None,
        };
        let messages = prepare_messages(&template, &config, &brain);
        assert!(messages.is_ok(), "Failed to prepare messages");
        let session_messages = messages.unwrap();
        let message_count =
            session_messages.preamble_messages.len() + session_messages.conversation_messages.len();
        assert_eq!(message_count, 4, "Unexpected number of messages");
    }

    // Add more specific test cases to handle different scenarios and edge cases
}
