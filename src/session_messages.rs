//! # Session Messages Module
//!
//! Manages the lifecycle of chat session messages, including:
//!
//! - Persistence to a SQLite database (via Diesel)
//! - Token counting (tiktoken) to enforce context budgets
//! - Serialization to/from `async-openai` chat message types
//! - Ejection decisions when context is too large
//!
//! ## What this module owns
//! - A `SessionMessages` struct that holds **preamble** messages (system/brain/template) and
//!   **conversation** messages (user/assistant), plus DB connectivity and config.
//! - Helpers to insert/query conversations and messages.
//! - Utilities to convert to/from OpenAI chat types, and to count tokens.
//!
//! ## Typical flow (high level)
//! 1. Create `SessionMessages::new(config)`.
//! 2. Load preamble and prior conversation from DB, or build a fresh preamble.
//! 3. Push new user message; decide whether to stream or fetch a completion.
//! 4. Persist assistant reply; update token counts and possibly eject old turns.
//!
//! ## Diesel schema
//! This module expects the standard Awful Jade Diesel schema with `conversations` and `messages`
//! tables (see `crate::schema`). A valid `session_name` must be present in
//! `AwfulJadeConfig` to associate records.

use async_openai::types::ChatCompletionRequestAssistantMessage;
use async_openai::types::ChatCompletionRequestAssistantMessageContent;
use async_openai::types::ChatCompletionRequestSystemMessageContent;
use async_openai::types::ChatCompletionRequestUserMessage;
use async_openai::types::ChatCompletionRequestUserMessageContent;
use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, Role};
use diesel::{Connection, SqliteConnection};

use crate::{
    config::{AwfulJadeConfig, establish_connection},
    models::{Conversation, Message},
};

use diesel::prelude::*;
use tiktoken_rs::cl100k_base;

/// Container for all messages in the current session plus DB connectivity.
///
/// Holds:
/// - `preamble_messages`: System/brain/template messages that always lead the prompt.
/// - `conversation_messages`: The rolling user/assistant exchange for this turn.
/// - `config`: Copy of `AwfulJadeConfig` for token budgets and DB URL.
/// - `sqlite_connection`: Live connection used for persistence.
pub struct SessionMessages {
    /// Messages that form the system preamble, including instructions and initial memory.
    pub preamble_messages: Vec<ChatCompletionRequestMessage>,

    /// Ongoing conversation messages between user and assistant.
    pub conversation_messages: Vec<ChatCompletionRequestMessage>,

    /// Application configuration (including token limits).
    config: AwfulJadeConfig,

    /// Live SQLite connection for persisting session data.
    sqlite_connection: SqliteConnection,
}

impl SessionMessages {
    /// Create a new `SessionMessages` from an application config.
    ///
    /// Establishes a SQLite connection immediately using `config.session_db_url`.
    ///
    /// # Parameters
    /// - `config`: Application configuration (cloned internally).
    ///
    /// # Returns
    /// A new `SessionMessages` with empty message buffers.
    ///
    /// # Panics
    /// Panics if the SQLite connection cannot be established.
    ///
    /// # Examples
    /// ```no_run
    /// use awful_aj::config::AwfulJadeConfig;
    /// use awful_aj::session_messages::SessionMessages;
    ///
    /// let cfg = AwfulJadeConfig {
    ///     api_key: String::new(),
    ///     api_base: String::new(),
    ///     model: "demo".into(),
    ///     context_max_tokens: 8192,
    ///     assistant_minimum_context_tokens: 2048,
    ///     stop_words: vec![],
    ///     session_db_url: "aj.db".into(),
    ///     session_name: Some("my-session".into()),
    ///     should_stream: None,
    /// };
    /// let sess = SessionMessages::new(cfg);
    /// ```
    pub fn new(config: AwfulJadeConfig) -> Self {
        Self {
            preamble_messages: Vec::new(),
            conversation_messages: Vec::new(),
            config: config.clone(),
            sqlite_connection: establish_connection(&config.session_db_url),
        }
    }

    /// Serialize a chat message into the database `Message` model.
    ///
    /// This does **not** write to the database; it only builds the struct
    /// (use [`persist_message`] to insert it).
    ///
    /// # Parameters
    /// - `role`: String role (`"system"`, `"user"`, `"assistant"`).
    /// - `content`: Text content.
    /// - `dynamic`: Whether the message was generated dynamically.
    /// - `conversation`: The conversation the message belongs to.
    ///
    /// # Returns
    /// A `Message` ready for insertion.
    pub fn serialize_chat_message(
        role: String,
        content: String,
        dynamic: bool,
        conversation: &Conversation,
    ) -> Message {
        Message {
            id: None,
            role,
            content,
            dynamic,
            conversation_id: Some(conversation.id.unwrap()),
        }
    }

    /// Convert a `Role` plus `content` into an OpenAI chat message.
    ///
    /// Supported roles: `System`, `User`, `Assistant`. Other roles produce `None`,
    /// and this function **unwraps** the result, so it will **panic** on unsupported roles.
    ///
    /// # Parameters
    /// - `role`: Sender role.
    /// - `content`: Message content.
    ///
    /// # Returns
    /// A `ChatCompletionRequestMessage` corresponding to the role/content.
    ///
    /// # Panics
    /// Panics if `role` is not one of `System | User | Assistant`.
    #[allow(deprecated)]
    pub fn serialize_chat_completion_message(
        role: Role,
        content: String,
    ) -> ChatCompletionRequestMessage {
        let message = match role {
            Role::System => Some(ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: ChatCompletionRequestSystemMessageContent::Text(content.clone()),
                    name: None,
                },
            )),
            Role::User => Some(ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(content.clone()),
                    name: None,
                },
            )),
            Role::Assistant => Some(ChatCompletionRequestMessage::Assistant(
                ChatCompletionRequestAssistantMessage {
                    content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                        content.clone(),
                    )),
                    name: None,
                    refusal: None,
                    audio: None,
                    tool_calls: None,
                    function_call: None,
                },
            )),
            _ => None,
        };

        message.unwrap()
    }

    /// Insert a single `Message` row into the database.
    ///
    /// Runs in a transaction and returns the inserted record (with ID).
    ///
    /// # Parameters
    /// - `message`: The message to persist (usually built via [`serialize_chat_message`]).
    ///
    /// # Returns
    /// `Ok(Message)` with the returned row, or `Err(diesel::result::Error)` on failure.
    pub fn persist_message(&mut self, message: &Message) -> Result<Message, diesel::result::Error> {
        let message: Message = self.sqlite_connection.transaction(|conn| {
            diesel::insert_into(crate::schema::messages::table)
                .values(message)
                .returning(Message::as_returning())
                .get_result(conn)
        })?;

        Ok(message)
    }

    /// Persist a batch of `ChatCompletionRequestMessage`s to the database.
    ///
    /// The current conversation is determined via [`query_conversation`]. Each chat message
    /// is converted to a DB `Message` and inserted within its own transaction.
    ///
    /// # Parameters
    /// - `messages`: The chat messages to persist.
    ///
    /// # Returns
    /// Vector of inserted `Message` rows, or an error if any insert fails.
    ///
    /// # Panics
    /// Panics if there is no active conversation (because `query_conversation()` is unwrapped).
    pub fn persist_chat_completion_messages(
        &mut self,
        messages: &Vec<ChatCompletionRequestMessage>,
    ) -> Result<Vec<Message>, diesel::result::Error> {
        let mut persisted_messages = Vec::new();
        let conversation = self.query_conversation().unwrap();

        for message in messages {
            let (role, content) = match message {
                ChatCompletionRequestMessage::System(system_message) => {
                    if let ChatCompletionRequestSystemMessageContent::Text(system_message_content) =
                        system_message.content.clone()
                    {
                        (Some(Role::System), Some(system_message_content))
                    } else {
                        (None, None)
                    }
                }
                ChatCompletionRequestMessage::User(user_message) => {
                    if let ChatCompletionRequestUserMessageContent::Text(user_message_content) =
                        user_message.content.clone()
                    {
                        (Some(Role::User), Some(user_message_content))
                    } else {
                        (None, None)
                    }
                }
                ChatCompletionRequestMessage::Assistant(assistant_message) => {
                    if let Some(ChatCompletionRequestAssistantMessageContent::Text(
                        assistant_message_content,
                    )) = assistant_message.content.clone()
                    {
                        (Some(Role::Assistant), Some(assistant_message_content))
                    } else {
                        (None, None)
                    }
                }
                _ => (None, None),
            };

            let chat_message = Self::serialize_chat_message(
                role.expect("Serializing messages requires a Role")
                    .to_string(),
                content.expect("Serializing messages requires message content"),
                false,
                &conversation,
            );
            let ret = self.persist_message(&chat_message);
            persisted_messages.push(ret.unwrap());
        }

        Ok(persisted_messages)
    }

    /// Insert a single message into the current conversation.
    ///
    /// # Parameters
    /// - `role`: Role as a string (`"system"`, `"user"`, `"assistant"`).
    /// - `content`: Message text.
    ///
    /// # Returns
    /// The inserted `Message` row, or an error if the conversation could not be found or insert fails.
    pub fn insert_message(
        &mut self,
        role: String,
        content: String,
    ) -> Result<Message, diesel::result::Error> {
        let conversation = self.query_conversation();

        match conversation {
            Ok(convo) => {
                let chat_message = Self::serialize_chat_message(role, content, false, &convo);

                self.persist_message(&chat_message)
            }
            Err(err) => Err(err),
        }
    }

    /// Look up the active conversation based on `config.session_name`.
    ///
    /// # Returns
    /// - `Ok(Conversation)` if found.
    /// - `Err(NotFound)` if `session_name` is `None` or the row does not exist.
    pub fn query_conversation(&mut self) -> Result<Conversation, diesel::result::Error> {
        let a_session_name = self.config.session_name.as_ref();

        if a_session_name.is_none() {
            return Err(diesel::result::Error::NotFound);
        }

        let conversation: Result<Conversation, diesel::result::Error> =
            self.sqlite_connection.transaction(|conn| {
                let existing_conversation: Result<Conversation, diesel::result::Error> =
                    crate::schema::conversations::table
                        .filter(
                            crate::schema::conversations::session_name.eq(a_session_name.unwrap()),
                        )
                        .first(conn);

                existing_conversation
            });

        conversation
    }

    /// Fetch all messages that belong to a conversation.
    ///
    /// # Parameters
    /// - `conversation`: Conversation to query by ID.
    ///
    /// # Returns
    /// Vector of `Message` in that conversation, ordered by default Diesel behavior.
    pub fn query_conversation_messages(
        &mut self,
        conversation: &Conversation,
    ) -> Result<Vec<Message>, diesel::result::Error> {
        let messages: Result<Vec<Message>, diesel::result::Error> =
            self.sqlite_connection.transaction(|conn| {
                let recent_messages: Result<Vec<Message>, diesel::result::Error> =
                    crate::schema::messages::table
                        .filter(crate::schema::messages::conversation_id.eq(conversation.id))
                        .load(conn);

                recent_messages
            });

        messages
    }

    /// Convert a string role to an OpenAI `Role`.
    ///
    /// Accepted: `"system"`, `"user"`, `"assistant"`.
    ///
    /// # Panics
    /// Panics on any unrecognized role string.
    ///
    /// # Parameters
    /// - `role`: Role as a `&str`.
    ///
    /// # Returns
    /// A `Role` enum variant.
    pub fn string_to_role(role: &str) -> Role {
        match role {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            err => panic!("Role in message not allowed: {err}"),
        }
    }

    /// Count tokens in a single `Message` (using `cl100k_base` tokenizer).
    ///
    /// # Parameters
    /// - `message`: The DB message whose `content` is tokenized.
    ///
    /// # Returns
    /// Number of tokens as `isize`.
    pub fn count_tokens_in_message(message: &Message) -> isize {
        let bpe = cl100k_base().unwrap();
        let msg_tokens = bpe.encode_with_special_tokens(&message.content);

        msg_tokens.len() as isize
    }

    /// Count tokens in a set of `ChatCompletionRequestMessage`s.
    ///
    /// Only counts the textual content of `System`, `User`, and textual `Assistant` messages.
    ///
    /// # Parameters
    /// - `messages`: The in-memory OpenAI messages to sum.
    ///
    /// # Returns
    /// Sum of tokens across all messages as `isize`.
    pub fn count_tokens_in_chat_completion_messages(
        messages: &Vec<ChatCompletionRequestMessage>,
    ) -> isize {
        let bpe = cl100k_base().unwrap();
        let mut count: isize = 0;
        for msg in messages {
            let content = match msg {
                ChatCompletionRequestMessage::System(system_message) => {
                    if let ChatCompletionRequestSystemMessageContent::Text(system_message_content) =
                        system_message.content.clone()
                    {
                        Some(system_message_content)
                    } else {
                        None
                    }
                }
                ChatCompletionRequestMessage::User(user_message) => {
                    if let ChatCompletionRequestUserMessageContent::Text(user_message_content) =
                        user_message.content.clone()
                    {
                        Some(user_message_content)
                    } else {
                        None
                    }
                }
                ChatCompletionRequestMessage::Assistant(assistant_message) => {
                    if let Some(ChatCompletionRequestAssistantMessageContent::Text(
                        assistant_message_content,
                    )) = assistant_message.content.clone()
                    {
                        Some(assistant_message_content)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(content) = content {
                let msg_tokens = bpe.encode_with_special_tokens(&content);
                count += msg_tokens.len() as isize;
            }
        }

        count
    }

    /// Compute remaining token budget **before** older messages must be ejected.
    ///
    /// This uses:
    /// - Token count of the current in-memory `preamble_messages`, and
    /// - Token count of the provided `messages` (often the DB-backed history you plan to include).
    ///
    /// # Parameters
    /// - `messages`: Messages to consider for inclusion.
    ///
    /// # Returns
    /// Remaining tokens (`max_tokens - used_tokens`) as `isize` (may be negative).
    pub fn tokens_left_before_ejection(&self, messages: Vec<Message>) -> isize {
        let bpe = cl100k_base().unwrap();
        let max_tokens =
            (self.config.context_max_tokens as i32) - self.config.assistant_minimum_context_tokens;

        let premable_tokens =
            Self::count_tokens_in_chat_completion_messages(&self.preamble_messages);

        let mut rest_of_convo_tokens: isize = 0;
        for msg in messages {
            let msg_tokens = bpe.encode_with_special_tokens(&msg.content);
            rest_of_convo_tokens += msg_tokens.len() as isize;
        }

        let tokens_in_session = premable_tokens + rest_of_convo_tokens;

        max_tokens as isize - tokens_in_session
    }

    /// Maximum token budget available to the assistant for the whole session.
    ///
    /// Computed as `context_max_tokens - assistant_minimum_context_tokens`.
    ///
    /// # Returns
    /// The token budget as `isize`.
    pub fn max_tokens(&self) -> isize {
        ((self.config.context_max_tokens as i32) - self.config.assistant_minimum_context_tokens)
            as isize
    }

    /// Should we eject old messages right now?
    ///
    /// Compares the tokens in **current** `preamble_messages + conversation_messages`
    /// against the session budget.
    ///
    /// # Returns
    /// `true` if total tokens exceed the session budget; otherwise `false`.
    pub fn should_eject_message(&self) -> bool {
        let session_token_count =
            Self::count_tokens_in_chat_completion_messages(&self.preamble_messages)
                + Self::count_tokens_in_chat_completion_messages(&self.conversation_messages);
        tracing::info!("SESSION TOKEN COUNT: {}", session_token_count);
        tracing::info!("ALLOTTED TOKENS {}", self.max_tokens());

        session_token_count > self.max_tokens()
    }
}
