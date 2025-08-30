//! # API Module
//!
//! High-level chat plumbing for Awful Jade. This module wires together:
//!
//! - An **OpenAI-compatible** client (via `async_openai`).
//! - **Session messages** (system/template messages + rolling conversation).
//! - Optional **semantic memory** via a [`VectorStore`] and a [`Brain`].
//! - Either **streaming** or **non-streaming** completions, selected by
//!   [`AwfulJadeConfig::should_stream`].
//!
//! ## What this module does
//! 1. **Builds the API client** from config ([`create_client`]).
//! 2. **Prepares the message stack** depending on whether we have an existing session
//!    ([`get_session_messages`], [`prepare_messages_for_existing_session`], [`prepare_messages`]).
//! 3. **Optionally injects memory** retrieved from the vector store into the brain
//!    ([`add_memories_to_brain`]).
//! 4. **Calls the chat API** either in streaming mode ([`stream_response`]) or one-shot mode
//!    ([`fetch_response`]) and **prints** (streaming) / **collects** (non-streaming) the content.
//! 5. **Persists** assistant messages to the session DB when sessions are enabled.
//!
//! ## Sessions & memory
//! If the configuration has `session_name: Some(...)`, the module:
//! - Loads recent messages from the session DB (or seeds a new session).
//! - When the token budget is exceeded, **ejects the oldest message pair** (user + assistant),
//!   embeds them, and stores them in the [`VectorStore`]. The index is rebuilt after successful
//!   inserts (HNSW build is cheap at small scales).
//! - Retrieves nearby memories for the new query and injects them into the [`Brain`] preamble,
//!   respecting the brain’s token budget configured by the caller.
//!
//! ## Streaming vs. non-streaming
//! - When `config.should_stream == Some(true)`, [`stream_response`] is used. Tokens appear live
//!   on stdout with lightweight color formatting, and the final assistant message is returned.
//! - Otherwise [`fetch_response`] is used to perform a single request/response.
//!
//! ## Example
//! ```no_run
//! use awful_aj::api::ask;
//! use awful_aj::config::AwfulJadeConfig;
//! use awful_aj::template::ChatTemplate;
//!
//! # async fn demo(cfg: AwfulJadeConfig, tpl: ChatTemplate) -> anyhow::Result<()> {
//! let answer = ask(&cfg, "What is the meaning of life?".into(), &tpl, None, None).await?;
//! println!("assistant said: {answer}");
//! # Ok(())
//! # }
//! ```

use crate::{
    brain::{Brain, Memory},
    config::{AwfulJadeConfig, establish_connection},
    session_messages::SessionMessages,
    template::ChatTemplate,
    vector_store::VectorStore,
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs, ResponseFormat,
        Role,
    },
};
use crossterm::{
    ExecutableCommand,
    cursor::MoveTo,
    style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
};
use futures::StreamExt;
use hora::core::{ann_index::ANNIndex, node::Node};
use std::{
    env,
    error::Error,
    io::{Write, stdout},
    thread,
    time::Duration,
};

use tracing::{debug, error};

/// Create an OpenAI-compatible client from [`AwfulJadeConfig`].
///
/// - Uses `api_base` and `api_key`.
/// - No retries are performed here; upstream code handles retry/stream policies.
///
/// # Errors
/// Returns an error if client initialization fails (unlikely unless invalid config).
///
/// # Example
/// ```no_run
/// # use awful_aj::config::AwfulJadeConfig;
/// # async fn demo(cfg: AwfulJadeConfig) -> anyhow::Result<()> {
/// let client = awful_aj::api::tests::create_client_for_docs(&cfg)?; // internal helper in tests
/// # Ok(()) }
/// ```
fn create_client(config: &AwfulJadeConfig) -> Result<Client<OpenAIConfig>, Box<dyn Error>> {
    let openai_config = OpenAIConfig::new()
        .with_api_key(config.api_key.clone())
        .with_api_base(config.api_base.clone());
    debug!("Client created with config: {:?}", openai_config);
    Ok(Client::with_config(openai_config))
}

/// Stream assistant tokens to stdout and return the final assistant message.
///
/// Behavior:
/// - While the session exceeds its token budget, **eject** the oldest user/assistant pair,
///   embed them, and store them in the vector store (if provided), then **rebuild** the index.
/// - Compose request messages from `preamble_messages + conversation_messages`.
/// - If the template specifies a JSON schema response format, it is forwarded to the API.
/// - Print streamed tokens in blue/bold as they arrive.
/// - Return a well-formed `Assistant` message containing the full streamed text.
///
/// # Parameters
/// - `client`: OpenAI client.
/// - `model`: Model identifier (as accepted by your server).
/// - `session_messages`: Mutable session state (preamble + rolling conversation).
/// - `config`: App config (max tokens, stop words, etc.).
/// - `template`: The chat template that may carry a `response_format`.
/// - `vector_store`: Optional semantic memory store (for ejecting/adding memories).
/// - `_brain`: Optional brain (unused here; memory injection happens before the call).
///
/// # Errors
/// - Network/API errors when creating the stream.
/// - I/O errors when writing to stdout.
/// - Embedding/indexing errors if the vector store fails to add/build.
///
/// # Panics
/// - Will `unwrap()` when writing to the locked stdout (operationally safe in TTYs).
#[allow(deprecated)]
async fn stream_response<'a>(
    client: &Client<OpenAIConfig>,
    model: String,
    session_messages: &mut SessionMessages,
    config: &AwfulJadeConfig,
    template: &ChatTemplate,
    mut vector_store: Option<&mut VectorStore>,
    _brain: Option<&mut Brain<'a>>,
) -> Result<ChatCompletionRequestMessage, Box<dyn Error>> {
    while session_messages.should_eject_message() {
        if !session_messages.conversation_messages.is_empty() {
            let ejected_user_message = session_messages.conversation_messages.remove(0);
            let ejected_assistant_message = session_messages.conversation_messages.remove(0);

            if let Some(the_vector_store) = vector_store.as_deref_mut() {
                if let ChatCompletionRequestMessage::User(user_message) = ejected_user_message {
                    if let ChatCompletionRequestUserMessageContent::Text(user_message_content) =
                        user_message.content
                    {
                        let vector =
                            the_vector_store.embed_text_to_vector(&user_message_content)?;
                        let memory = Memory::new(Role::User, user_message_content);
                        if the_vector_store
                            .add_vector_with_content(vector, memory)
                            .is_ok()
                        {
                            the_vector_store.build()?;
                        }
                    }
                };

                if let ChatCompletionRequestMessage::Assistant(assistant_message) =
                    ejected_assistant_message
                {
                    if let Some(ChatCompletionRequestAssistantMessageContent::Text(
                        assistant_message_content,
                    )) = assistant_message.content
                    {
                        let vector =
                            the_vector_store.embed_text_to_vector(&assistant_message_content)?;
                        let memory = Memory::new(Role::User, assistant_message_content);
                        if the_vector_store
                            .add_vector_with_content(vector, memory)
                            .is_ok()
                        {
                            the_vector_store.build()?;
                        }
                    }
                };
            }
        } else {
            break;
        }
    }

    let mut full_conversation = Vec::new();
    full_conversation.append(&mut session_messages.preamble_messages);
    full_conversation.append(&mut session_messages.conversation_messages);

    let request = match template.response_format.clone() {
        Some(response_format_json_schema) => {
            let response_format = ResponseFormat::JsonSchema {
                json_schema: response_format_json_schema,
            };

            CreateChatCompletionRequestArgs::default()
                .max_tokens(config.context_max_tokens)
                .model(model)
                .stop(config.stop_words.clone())
                .messages(full_conversation)
                .response_format(response_format)
                .build()?
        }
        None => CreateChatCompletionRequestArgs::default()
            .max_tokens(config.context_max_tokens)
            .model(model)
            .stop(config.stop_words.clone())
            .messages(full_conversation)
            .build()?,
    };

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
                        write!(lock, "{content}").unwrap();
                    }
                });
            }
            Err(err) => {
                error!("Received error: {}", err);
                writeln!(lock, "error: {err}").unwrap();
            }
        }
        stdout.flush()?;
    }

    stdout.execute(SetAttribute(Attribute::Reset))?;
    stdout.execute(SetForegroundColor(Color::Reset))?;

    drop(lock);

    let chat_completion_request_assistant_content =
        ChatCompletionRequestAssistantMessageContent::Text(response_string.clone());

    let chat_completion_request_message =
        ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
            content: Some(chat_completion_request_assistant_content),
            name: None,
            refusal: None,
            audio: None,
            tool_calls: None,
            function_call: None,
        });

    Ok(chat_completion_request_message)
}

/// Non-streaming chat: send a single request and return the assistant message.
///
/// This mirrors the eject-and-store behavior from [`stream_response`], but performs a
/// single `create` call and aggregates the returned content into one `Assistant` message.
///
/// # Errors
/// Propagates API, I/O, embedding, and index-build errors.
#[allow(clippy::collapsible_match, deprecated)]
async fn fetch_response<'a>(
    client: &Client<OpenAIConfig>,
    model: String,
    session_messages: &mut SessionMessages,
    config: &AwfulJadeConfig,
    template: &ChatTemplate,
    mut vector_store: Option<&mut VectorStore>,
    _brain: Option<&mut Brain<'a>>,
) -> Result<ChatCompletionRequestMessage, Box<dyn Error>> {
    while session_messages.should_eject_message() {
        if !session_messages.conversation_messages.is_empty() {
            let ejected_user_message = session_messages.conversation_messages.remove(0);

            let ejected_assistant_message = if !session_messages.conversation_messages.is_empty() {
                Some(session_messages.conversation_messages.remove(0))
            } else {
                None
            };

            if let Some(the_vector_store) = vector_store.as_deref_mut() {
                if let ChatCompletionRequestMessage::User(user_message) = ejected_user_message {
                    if let ChatCompletionRequestUserMessageContent::Text(user_message_content) =
                        user_message.content
                    {
                        let vector =
                            the_vector_store.embed_text_to_vector(&user_message_content)?;
                        let memory = Memory::new(Role::User, user_message_content);
                        if the_vector_store
                            .add_vector_with_content(vector, memory)
                            .is_ok()
                        {
                            the_vector_store.build()?;
                        }
                    }
                };

                if let Some(ejected_assistant_message) = ejected_assistant_message {
                    if let ChatCompletionRequestMessage::Assistant(assistant_message) =
                        ejected_assistant_message
                    {
                        if let Some(ChatCompletionRequestAssistantMessageContent::Text(
                            assistant_message_content,
                        )) = assistant_message.content
                        {
                            let vector = the_vector_store
                                .embed_text_to_vector(&assistant_message_content)?;
                            let memory = Memory::new(Role::User, assistant_message_content);
                            if the_vector_store
                                .add_vector_with_content(vector, memory)
                                .is_ok()
                            {
                                the_vector_store.build()?;
                            }
                        }
                    };
                };
            }
        } else {
            break;
        }
    }

    let mut full_conversation = Vec::new();
    full_conversation.append(&mut session_messages.preamble_messages);
    full_conversation.append(&mut session_messages.conversation_messages);

    let request = match template.response_format.clone() {
        Some(response_format_json_schema) => {
            let response_format = ResponseFormat::JsonSchema {
                json_schema: response_format_json_schema,
            };

            CreateChatCompletionRequestArgs::default()
                .max_tokens(config.context_max_tokens)
                .model(model)
                .stop(config.stop_words.clone())
                .messages(full_conversation)
                .response_format(response_format)
                .build()?
        }
        None => CreateChatCompletionRequestArgs::default()
            .max_tokens(config.context_max_tokens)
            .model(model)
            .stop(config.stop_words.clone())
            .messages(full_conversation)
            .build()?,
    };

    debug!("Sending request: {:?}", request);

    let mut response_string = String::new();

    let response = client.chat().create(request).await?;

    response.choices.iter().for_each(|chat_choice| {
        let message = chat_choice.message.clone();
        let message_content = message.content;
        if let Some(message_text) = message_content {
            response_string.push_str(&message_text);
        }
    });

    let chat_completion_request_assistant_content =
        ChatCompletionRequestAssistantMessageContent::Text(response_string.clone());

    let chat_completion_request_message =
        ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
            content: Some(chat_completion_request_assistant_content),
            name: None,
            refusal: None,
            audio: None,
            tool_calls: None,
            function_call: None,
        });

    Ok(chat_completion_request_message)
}

use crate::api::ChatCompletionRequestAssistantMessageContent::Text;

/// Ask a single question and return the assistant’s textual answer.
///
/// Pipeline:
/// 1. Build client and session messages (loading session state when available).
/// 2. Optionally add nearest-neighbor memories from the vector store into the brain.
/// 3. Apply template `pre_user_message_content` / `post_user_message_content`.
/// 4. Send the request (streaming or non-streaming).
/// 5. Persist the assistant message to the session DB (when sessions are enabled).
///
/// # Parameters
/// - `config`: App configuration (API base/key, model, token budgets, etc.).
/// - `question`: User input.
/// - `template`: Chat template (system prompt + seed messages).
/// - `vector_store`: Optional vector store (used to fetch/store memories).
/// - `brain`: Optional brain (holds the working memory/preamble).
///
/// # Returns
/// The assistant’s textual content.
///
/// # Errors
/// Propagates API, I/O, (de)serialization, embedding, and DB errors.
///
/// # Example
/// ```no_run
/// # async fn demo(cfg: awful_aj::config::AwfulJadeConfig, tpl: awful_aj::template::ChatTemplate)
/// # -> anyhow::Result<()> {
/// let answer = awful_aj::api::ask(&cfg, "Ping?".into(), &tpl, None, None).await?;
/// println!("assistant: {answer}");
/// # Ok(()) }
/// ```
#[allow(clippy::collapsible_match)]
pub async fn ask<'a>(
    config: &AwfulJadeConfig,
    question: String,
    template: &ChatTemplate,
    vector_store: Option<&mut VectorStore>,
    mut brain: Option<&mut Brain<'a>>,
) -> Result<String, Box<dyn Error>> {
    let client = create_client(config)?;
    let mut session_messages = get_session_messages(&brain, config, template, &question).unwrap();
    let _added_memories_to_brain_result =
        add_memories_to_brain(&vector_store, &question, &mut session_messages, &mut brain);

    let mut question = if let Some(prepend_content) = template.pre_user_message_content.clone() {
        format!("{prepend_content} {question}")
    } else {
        question
    };

    question = if let Some(append_content) = template.post_user_message_content.clone() {
        format!("{question} {append_content}")
    } else {
        question
    };

    let chat_completion_request_message =
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(question),
            name: None,
        });

    session_messages
        .conversation_messages
        .push(chat_completion_request_message);

    let assistant_response: ChatCompletionRequestMessage = match config.should_stream {
        Some(true) => {
            stream_response(
                &client,
                config.model.clone(),
                &mut session_messages,
                config,
                template,
                vector_store,
                brain,
            )
            .await?
        }
        Some(false) | None => {
            fetch_response(
                &client,
                config.model.clone(),
                &mut session_messages,
                config,
                template,
                vector_store,
                brain,
            )
            .await?
        }
    };

    let assistant_message_content = match assistant_response {
        ChatCompletionRequestMessage::Assistant(assistant_message) => assistant_message.content,
        _ => None,
    };

    if let Some(assistant_response_content) = assistant_message_content {
        if let Text(assistant_response_content_text) = assistant_response_content {
            let _diesel_sqlite_response = session_messages.insert_message(
                "assistant".to_string(),
                assistant_response_content_text.clone(),
            );
            return Ok(assistant_response_content_text);
        }
    };

    Err("No assistant response".into())
}

/// Prepare session messages for a new or existing session.
///
/// - If `config.session_name.is_some()` **and** a brain is provided, we attempt to
///   load the conversation from the DB and persist the new user question immediately.
/// - Otherwise we create a fresh session with system + template messages. When a brain
///   is provided, its preamble (system + “Ok” handshake + internal state) is included.
///
/// # Errors
/// Returns DB/serialization errors when loading or persisting messages.
fn get_session_messages(
    brain: &Option<&mut Brain>,
    config: &AwfulJadeConfig,
    template: &ChatTemplate,
    question: &String,
) -> Result<SessionMessages, Box<dyn Error>> {
    let session_messages = if config.session_name.is_some() && brain.is_some() {
        let prepare_brain = brain.as_ref().expect("We need a Brain here!");
        let session_messages =
            prepare_messages_for_existing_session(template, config, prepare_brain)?;

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
        let prepare_brain = brain.as_ref();
        prepare_messages(template, config, prepare_brain).unwrap()
    };

    Ok(session_messages)
}

/// Retrieve nearby memories from the vector store and inject them into the brain.
///
/// Steps:
/// 1. Embed the query.
/// 2. `search_nodes` for the top-3 neighbors.
/// 3. If a neighbor’s **Euclidean distance** is `< 1.0`, add its content to the brain.
/// 4. Rebuild the brain preamble (so it lands in the current request).
///
/// # Notes
/// - This expects the vector store to map IDs → [`Memory`].
/// - Distance threshold (`< 1.0`) is empirical and can be tuned.
///
/// # Errors
/// Embedding/search errors, and preamble build errors (unlikely).
fn add_memories_to_brain(
    vector_store: &Option<&mut VectorStore>,
    question: &str,
    session_messages: &mut SessionMessages,
    brain: &mut Option<&mut Brain>,
) -> Result<(), Box<dyn Error>> {
    if let Some(vector_store) = vector_store {
        // Embed the user's input
        let vector = vector_store.embed_text_to_vector(question)?;

        // Query the VectorStore to get relevant content based on user's input
        let neighbor_vectors = vector_store.index.search_nodes(&vector, 3); // Adjust the number of neighbors as needed

        let neighbor_vec_distances = neighbor_vectors.iter().map(|v| {
            let (node, distance): (Node<f32, usize>, f32) = v.clone();
            (node.vectors().clone(), *node.idx(), distance)
        });

        for (_vector, id, euclidean_distance) in neighbor_vec_distances {
            if let Some(neighbor_content) = vector_store.get_content_by_id(id.unwrap()) {
                if let Some(brain) = brain {
                    if euclidean_distance < 1.0 {
                        brain.add_memory((*neighbor_content).clone(), session_messages);
                    }
                }
            }
        }

        if let Some(brain) = brain {
            session_messages.preamble_messages = brain.build_preamble().unwrap();
        }
    }

    Ok(())
}

/// Build a brand-new session message stack (no prior DB history).
///
/// Puts together:
/// - The **system** message (from the template).
/// - The **brain preamble** (system + brain state + assistant “Ok”), if a brain is supplied.
/// - Any **template messages** bundled with the template.
///
/// # Errors
/// Returns formatting/serialization errors (rare).
fn prepare_messages(
    template: &ChatTemplate,
    config: &AwfulJadeConfig,
    brain: Option<&&mut Brain>,
) -> Result<SessionMessages, Box<dyn Error>> {
    let mut session_messages = SessionMessages::new(config.clone());

    if let Some(brain) = brain {
        let mut preamble_messages = brain.build_preamble().unwrap();
        let mut template_messages = template.messages.clone();

        session_messages
            .preamble_messages
            .append(&mut preamble_messages);
        session_messages
            .preamble_messages
            .append(&mut template_messages);
    } else {
        let chat_completion_message =
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(
                    template.system_prompt.clone(),
                ),
                name: None,
            });

        let mut preamble_messages: Vec<ChatCompletionRequestMessage> =
            vec![chat_completion_message];
        let mut template_messages = template.messages.clone();

        session_messages
            .preamble_messages
            .append(&mut preamble_messages);
        session_messages
            .preamble_messages
            .append(&mut template_messages);
    }

    Ok(session_messages)
}

use crate::models::*;
use diesel::prelude::*;

#[allow(deprecated)]
/// Build/restore a session message stack from the DB, or seed a new one.
///
/// If the conversation exists:
/// - The first `3 + template.messages.len()` messages are treated as **preamble**
///   (system, brain JSON, assistant “Ok”, then the template messages).
/// - The remaining messages are loaded as **conversation** messages.
///
/// If the conversation does **not** exist:
/// - Build a fresh brain preamble and template messages, **persist** them to the DB,
///   and return the seeded `SessionMessages`.
///
/// # Errors
/// Returns DB errors when querying/persisting messages.
fn prepare_messages_for_existing_session(
    template: &ChatTemplate,
    config: &AwfulJadeConfig,
    brain: &&mut Brain,
) -> Result<SessionMessages, Box<dyn Error>> {
    let mut session_messages = SessionMessages::new(config.clone());

    let conversation: Result<Conversation, diesel::result::Error> =
        session_messages.query_conversation();

    match conversation {
        Ok(conversation) => {
            let recent_messages = session_messages.query_conversation_messages(&conversation);

            if let Ok(mut recent_msgs) = recent_messages {
                // Preamble = System Prompt, Brain Message, Assistant Acknowledgement, then N template messages
                if !recent_msgs.is_empty() {
                    let preamble_messages = recent_msgs.drain(0..(3 + template.messages.len()));
                    for msg in preamble_messages {
                        let role = SessionMessages::string_to_role(&msg.role);

                        let msg_obj = match role {
                            Role::System => ChatCompletionRequestMessage::System(
                                ChatCompletionRequestSystemMessage {
                                    content: ChatCompletionRequestSystemMessageContent::Text(
                                        msg.content.clone(),
                                    ),
                                    name: None,
                                },
                            ),
                            Role::User => ChatCompletionRequestMessage::User(
                                ChatCompletionRequestUserMessage {
                                    content: ChatCompletionRequestUserMessageContent::Text(
                                        msg.content.clone(),
                                    ),
                                    name: None,
                                },
                            ),
                            Role::Assistant => ChatCompletionRequestMessage::Assistant(
                                ChatCompletionRequestAssistantMessage {
                                    content: Some(
                                        ChatCompletionRequestAssistantMessageContent::Text(
                                            msg.content.clone(),
                                        ),
                                    ),
                                    name: None,
                                    refusal: None,
                                    audio: None,
                                    tool_calls: None,
                                    function_call: None,
                                },
                            ),
                            _ => panic!("We don't handle this Role yet!!"),
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

                        let (role, content) = match message {
                            ChatCompletionRequestMessage::System(system_message) => {
                                if let ChatCompletionRequestSystemMessageContent::Text(
                                    message_content,
                                ) = system_message.content
                                {
                                    (Some(Role::System), Some(message_content))
                                } else {
                                    (None, None)
                                }
                            }
                            ChatCompletionRequestMessage::User(user_message) => {
                                if let ChatCompletionRequestUserMessageContent::Text(
                                    message_content,
                                ) = user_message.content
                                {
                                    (Some(Role::User), Some(message_content))
                                } else {
                                    (None, None)
                                }
                            }
                            ChatCompletionRequestMessage::Assistant(assistant_message) => {
                                if let Some(ChatCompletionRequestAssistantMessageContent::Text(
                                    message_content,
                                )) = assistant_message.content
                                {
                                    (Some(Role::Assistant), Some(message_content))
                                } else {
                                    (None, None)
                                }
                            }
                            _ => (None, None),
                        };

                        if let Some(msg_content) = content {
                            let serialized_message = Message {
                                id: None,
                                role: role.unwrap().to_string(),
                                content: msg_content,
                                dynamic: false,
                                conversation_id: conversation.id,
                            };

                            let _res = session_messages.persist_message(&serialized_message);

                            session_messages.conversation_messages.push(msg_clone);
                        }
                    }
                }
            }

            Ok(session_messages)
        }
        Err(_) => {
            let prepare_brain = brain;
            prepare_messages(template, config, Some(prepare_brain))
        }
    }
}

use std::io::Read;

/// Interactive REPL loop.
///
/// Prints a styled `You:` prompt, reads from **stdin** (until EOF for the line),
/// builds/updates session messages, streams the assistant response, and persists it
/// to the session DB. Type `exit` to leave the loop.
///
/// **Note:** This reads with `stdin.read_to_string`, which consumes all available
/// stdin; when running in a terminal, provide input followed by EOF (Ctrl-D on Unix,
/// Ctrl-Z then Enter on Windows) or adapt to line-by-line reading if desired.
///
/// # Errors
/// Propagates API, I/O, and persistence errors.
#[allow(clippy::single_match)]
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
        std::io::stdin()
            .read_to_string(&mut input)
            .expect("Failed to read from stdin");

        stdout.execute(SetForegroundColor(Color::Reset))?;

        // Exit the loop if the user types "exit"
        if input.trim().to_lowercase() == "exit" {
            break;
        }

        let mut session_messages =
            get_session_messages(&Some(&mut brain), config, template, &input).unwrap();
        let _added_memories_to_brain_result = add_memories_to_brain(
            &Some(&mut vector_store),
            &input,
            &mut session_messages,
            &mut Some(&mut brain),
        );

        input = if let Some(prepend_content) = template.pre_user_message_content.clone() {
            format!("{prepend_content} {input}")
        } else {
            input
        };

        input = if let Some(append_content) = template.post_user_message_content.clone() {
            format!("{input} {append_content}")
        } else {
            input
        };

        let chat_completion_message =
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(input.to_string()),
                name: None,
            });

        session_messages
            .conversation_messages
            .push(chat_completion_message);

        // Get the AI's response using the OpenAI API
        let assistant_response = match stream_response(
            &client,
            config.model.clone(),
            &mut session_messages,
            config,
            template,
            Some(&mut vector_store),
            Some(&mut brain),
        )
        .await
        {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Error: {e}");
                continue; // This will skip the current iteration of the loop and proceed to the next one
            }
        };

        session_messages
            .conversation_messages
            .push(assistant_response.clone());

        match assistant_response {
            ChatCompletionRequestMessage::Assistant(assistant_message) => {
                if let Some(ChatCompletionRequestAssistantMessageContent::Text(
                    assistant_message_content,
                )) = assistant_message.content
                {
                    let _diesel_sqlite_response = session_messages
                        .insert_message("assistant".to_string(), assistant_message_content.clone());

                    unsafe { env::set_var("AJ", assistant_message_content) };
                }
            }
            _ => (),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber;

    fn setup() {
        let _ = tracing_subscriber::fmt::try_init();
    }

    /// Helper used in doc examples.
    pub(super) fn create_client_for_docs(
        cfg: &AwfulJadeConfig,
    ) -> Result<Client<OpenAIConfig>, Box<dyn Error>> {
        super::create_client(cfg)
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
            should_stream: None,
        }
    }

    // Mock template for testing
    fn mock_template() -> ChatTemplate {
        setup();

        let chat_completion_request =
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(
                    "How do I read a file in Rust?".to_string(),
                ),
                name: None,
            });

        ChatTemplate {
            system_prompt: "You are Awful Jade, a helpful AI assistant.".to_string(),
            messages: vec![chat_completion_request],
            response_format: None,
            pre_user_message_content: None,
            post_user_message_content: None,
        }
    }

    #[tokio::test]
    async fn test_create_client() {
        setup();
        let config = mock_config();
        let client = super::create_client(&config);
        assert!(client.is_ok(), "Failed to create client");
    }

    #[tokio::test]
    async fn test_prepare_messages() {
        setup();
        let template = mock_template();
        let mut brain = Brain::new(8092, &template);
        let config = AwfulJadeConfig {
            api_key: "".to_string(),
            api_base: "".to_string(),
            model: "".to_string(),
            context_max_tokens: 8092,
            assistant_minimum_context_tokens: 2048,
            stop_words: vec!["".to_string()],
            session_db_url: "".to_string(),
            session_name: None,
            should_stream: None,
        };
        let messages = super::prepare_messages(&template, &config, Some(&&mut brain));
        assert!(messages.is_ok(), "Failed to prepare messages");
        let session_messages = messages.unwrap();
        let message_count =
            session_messages.preamble_messages.len() + session_messages.conversation_messages.len();
        assert_eq!(message_count, 4, "Unexpected number of messages");
    }

    // Add more specific test cases to handle different scenarios and edge cases
}
