//! # Session Messages - Conversation Persistence & Lifecycle Management
//!
//! This module manages the complete lifecycle of chat conversations, bridging between:
//!
//! - **In-memory chat state**: OpenAI-compatible message structures
//! - **Persistent storage**: SQLite database via Diesel ORM
//! - **Token budgeting**: Context window management with tiktoken
//! - **Message ejection**: Automatic removal of old messages when context fills
//!
//! ## Overview
//!
//! [`SessionMessages`] is the primary type for managing conversation state. It maintains
//! two separate message queues:
//!
//! 1. **Preamble Messages**: System prompts, brain context, and template seeds that are
//!    always included at the start of every API call
//! 2. **Conversation Messages**: The rolling user/assistant exchange that grows over time
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                   SessionMessages                        │
//! │  ┌────────────────────┐  ┌────────────────────┐         │
//! │  │ Preamble Messages  │  │ Conversation Msgs  │         │
//! │  │  (System/Brain)    │  │  (User/Assistant)  │         │
//! │  │  [Always included] │  │  [Rolling window]  │         │
//! │  └────────────────────┘  └────────────────────┘         │
//! │              ↓                      ↓                    │
//! │         ┌─────────────────────────────────┐             │
//! │         │   Token Counting (tiktoken)     │             │
//! │         │   Context Budget Enforcement    │             │
//! │         └─────────────────────────────────┘             │
//! │              ↓                      ↓                    │
//! │    ┌─────────────────┐    ┌─────────────────┐          │
//! │    │  SQLite (Read)  │    │ SQLite (Write)  │          │
//! │    │  Load history   │    │ Persist new msg │          │
//! │    └─────────────────┘    └─────────────────┘          │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Capabilities
//!
//! | Feature | Implementation | Purpose |
//! |---------|---------------|---------|
//! | **Persistence** | Diesel ORM + SQLite | Save conversation history across sessions |
//! | **Token Counting** | tiktoken `cl100k_base` | Measure message size for budgeting |
//! | **Serialization** | `async-openai` types | Convert between DB models and API messages |
//! | **Ejection Logic** | `should_eject_message()` | Decide when to remove old messages |
//! | **Session Continuity** | `query_conversation()` | Resume conversations by session name |
//!
//! ## Typical Workflow
//!
//! ### 1. Initialize Session
//!
//! ```no_run
//! use awful_aj::{config::load_config, session_messages::SessionMessages};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = load_config("config.yaml")?;
//! let mut session = SessionMessages::new(config);
//! # Ok(())
//! # }
//! ```
//!
//! ### 2. Load Existing Conversation
//!
//! ```no_run
//! # use awful_aj::session_messages::SessionMessages;
//! # use awful_aj::config::AwfulJadeConfig;
//! # fn example(mut session: SessionMessages) -> Result<(), Box<dyn std::error::Error>> {
//! // Query conversation by session_name
//! let conversation = session.query_conversation()?;
//!
//! // Load all previous messages
//! let messages = session.query_conversation_messages(&conversation)?;
//!
//! println!("Loaded {} messages from session", messages.len());
//! # Ok(())
//! # }
//! ```
//!
//! ### 3. Add New Messages
//!
//! ```no_run
//! # use awful_aj::session_messages::SessionMessages;
//! # fn example(mut session: SessionMessages) -> Result<(), Box<dyn std::error::Error>> {
//! // Add user message
//! session.insert_message(
//!     "user".to_string(),
//!     "What is HNSW?".to_string(),
//! )?;
//!
//! // Add assistant response
//! session.insert_message(
//!     "assistant".to_string(),
//!     "HNSW is a graph-based algorithm...".to_string(),
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! ### 4. Token Budget Management
//!
//! ```no_run
//! # use awful_aj::session_messages::SessionMessages;
//! # fn example(session: SessionMessages) {
//! // Check if we need to eject old messages
//! if session.should_eject_message() {
//!     println!("Context window full - need to eject oldest messages");
//! }
//!
//! // Get remaining token budget
//! let budget = session.max_tokens();
//! println!("Total token budget: {}", budget);
//! # }
//! ```
//!
//! ## Token Counting
//!
//! Token counting uses OpenAI's `cl100k_base` tokenizer (same as GPT-4, GPT-3.5-turbo):
//!
//! ```no_run
//! use awful_aj::session_messages::SessionMessages;
//! use async_openai::types::{ChatCompletionRequestMessage, Role};
//!
//! # fn example() {
//! let messages = vec![
//!     SessionMessages::serialize_chat_completion_message(
//!         Role::User,
//!         "What is Rust?".to_string(),
//!     ),
//! ];
//!
//! let token_count = SessionMessages::count_tokens_in_chat_completion_messages(&messages);
//! println!("User prompt uses {} tokens", token_count);
//! # }
//! ```
//!
//! ## Ejection Strategy
//!
//! When the conversation exceeds the token budget, old messages must be ejected:
//!
//! 1. **Budget Calculation**: `context_max_tokens - assistant_minimum_context_tokens`
//! 2. **Usage Tracking**: Sum tokens in preamble + conversation messages
//! 3. **Ejection Trigger**: `should_eject_message()` returns `true` when budget exceeded
//! 4. **Ejection Policy**: Typically FIFO (remove oldest conversation messages first)
//!
//! **Note**: Preamble messages are **never ejected** - they always stay in context.
//!
//! ## Database Schema
//!
//! This module requires the Diesel schema defined in [`crate::schema`]:
//!
//! | Table | Purpose | Key Fields |
//! |-------|---------|-----------|
//! | `conversations` | Named sessions | `id`, `session_name` |
//! | `messages` | Individual turns | `id`, `role`, `content`, `conversation_id` |
//!
//! See [`crate::models`] for the ORM model definitions.
//!
//! ## Examples
//!
//! ### Complete Conversation Lifecycle
//!
//! ```no_run
//! use awful_aj::{config::load_config, session_messages::SessionMessages};
//! use async_openai::types::{ChatCompletionRequestMessage, Role};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize
//! let mut config = load_config("config.yaml")?;
//! config.session_name = Some("research-session".to_string());
//! let mut session = SessionMessages::new(config);
//!
//! // Set up preamble
//! session.preamble_messages.push(
//!     SessionMessages::serialize_chat_completion_message(
//!         Role::System,
//!         "You are a helpful research assistant.".to_string(),
//!     ),
//! );
//!
//! // Add user query
//! session.insert_message(
//!     "user".to_string(),
//!     "Explain vector databases".to_string(),
//! )?;
//!
//! // Simulate assistant response
//! let assistant_reply = "Vector databases store embeddings...";
//! session.insert_message(
//!     "assistant".to_string(),
//!     assistant_reply.to_string(),
//! )?;
//!
//! // Check token usage
//! if session.should_eject_message() {
//!     println!("Need to eject old messages!");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Batch Message Persistence
//!
//! ```no_run
//! # use awful_aj::session_messages::SessionMessages;
//! # use async_openai::types::{ChatCompletionRequestMessage, Role};
//! # fn example(mut session: SessionMessages) -> Result<(), Box<dyn std::error::Error>> {
//! let messages = vec![
//!     SessionMessages::serialize_chat_completion_message(
//!         Role::User,
//!         "First question".to_string(),
//!     ),
//!     SessionMessages::serialize_chat_completion_message(
//!         Role::Assistant,
//!         "First answer".to_string(),
//!     ),
//! ];
//!
//! // Persist all at once
//! let persisted = session.persist_chat_completion_messages(&messages)?;
//! println!("Persisted {} messages", persisted.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## See Also
//!
//! - [`crate::brain`] - Working memory with token budgeting (short-term memory)
//! - [`crate::vector_store`] - Semantic search over ejected messages (long-term memory)
//! - [`crate::models`] - Database ORM models (`Conversation`, `Message`)
//! - [`crate::schema`] - Auto-generated Diesel schema
//! - [`crate::api`] - API client that consumes session messages

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
        let max_tokens = (self.config.context_max_tokens as isize)
            - (self.config.assistant_minimum_context_tokens as isize);

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
        (self.config.context_max_tokens as isize)
            - (self.config.assistant_minimum_context_tokens as isize)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AwfulJadeConfig;

    /// Creates a test configuration for session messages tests
    fn create_test_config() -> AwfulJadeConfig {
        AwfulJadeConfig {
            api_key: "test_key".to_string(),
            api_base: "http://localhost:5001/v1".to_string(),
            model: "test_model".to_string(),
            context_max_tokens: 4096,
            assistant_minimum_context_tokens: 1024,
            stop_words: vec![],
            session_db_url: ":memory:".to_string(), // Use in-memory database for tests
            session_name: Some("test_session".to_string()),
            should_stream: Some(false),
        }
    }

    #[test]
    fn test_session_messages_creation() {
        let config = create_test_config();
        let session = SessionMessages::new(config.clone());

        assert_eq!(session.preamble_messages.len(), 0);
        assert_eq!(session.conversation_messages.len(), 0);
        assert_eq!(session.config.model, config.model);
    }

    #[test]
    fn test_serialize_chat_completion_message_user() {
        let msg = SessionMessages::serialize_chat_completion_message(
            Role::User,
            "Hello world".to_string(),
        );

        match msg {
            ChatCompletionRequestMessage::User(user_msg) => {
                if let ChatCompletionRequestUserMessageContent::Text(content) = user_msg.content {
                    assert_eq!(content, "Hello world");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn test_serialize_chat_completion_message_assistant() {
        let msg = SessionMessages::serialize_chat_completion_message(
            Role::Assistant,
            "I can help".to_string(),
        );

        match msg {
            ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                if let Some(ChatCompletionRequestAssistantMessageContent::Text(content)) =
                    assistant_msg.content
                {
                    assert_eq!(content, "I can help");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_serialize_chat_completion_message_system() {
        let msg = SessionMessages::serialize_chat_completion_message(
            Role::System,
            "You are helpful".to_string(),
        );

        match msg {
            ChatCompletionRequestMessage::System(system_msg) => {
                if let ChatCompletionRequestSystemMessageContent::Text(content) = system_msg.content
                {
                    assert_eq!(content, "You are helpful");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected System message"),
        }
    }

    #[test]
    fn test_string_to_role_conversions() {
        assert_eq!(SessionMessages::string_to_role("system"), Role::System);
        assert_eq!(SessionMessages::string_to_role("user"), Role::User);
        assert_eq!(
            SessionMessages::string_to_role("assistant"),
            Role::Assistant
        );
    }

    #[test]
    #[should_panic(expected = "Role in message not allowed")]
    fn test_string_to_role_invalid() {
        SessionMessages::string_to_role("invalid_role");
    }

    #[test]
    fn test_count_tokens_in_chat_completion_messages() {
        let messages = vec![
            SessionMessages::serialize_chat_completion_message(Role::User, "Hello".to_string()),
            SessionMessages::serialize_chat_completion_message(
                Role::Assistant,
                "Hi there!".to_string(),
            ),
        ];

        let token_count = SessionMessages::count_tokens_in_chat_completion_messages(&messages);

        // Should have some tokens (exact count depends on tiktoken)
        assert!(token_count > 0);
        assert!(token_count < 100); // Sanity check - these short messages shouldn't be huge
    }

    #[test]
    fn test_count_tokens_empty_messages() {
        let messages = vec![];

        let token_count = SessionMessages::count_tokens_in_chat_completion_messages(&messages);

        assert_eq!(token_count, 0);
    }

    #[test]
    fn test_max_tokens_calculation() {
        let config = create_test_config();
        let session = SessionMessages::new(config);

        let max = session.max_tokens();

        // Should be context_max_tokens - assistant_minimum_context_tokens
        assert_eq!(max, 4096 - 1024);
    }

    #[test]
    fn test_should_eject_message_under_budget() {
        let config = create_test_config();
        let mut session = SessionMessages::new(config);

        // Add a single short message
        session
            .preamble_messages
            .push(SessionMessages::serialize_chat_completion_message(
                Role::System,
                "Short".to_string(),
            ));

        // Should not need ejection with just one short message
        assert!(!session.should_eject_message());
    }

    #[test]
    fn test_should_eject_message_over_budget() {
        let mut config = create_test_config();
        // Set a very small token budget
        config.context_max_tokens = 100;
        config.assistant_minimum_context_tokens = 10;

        let mut session = SessionMessages::new(config);

        // Add many long messages to exceed budget
        for i in 0..50 {
            session.conversation_messages.push(
                SessionMessages::serialize_chat_completion_message(
                    Role::User,
                    format!(
                        "This is a long message number {} that will help us exceed the token budget",
                        i
                    ),
                ),
            );
        }

        // Should need ejection with many long messages
        assert!(session.should_eject_message());
    }

    #[test]
    fn test_tokens_left_before_ejection() {
        let config = create_test_config();
        let session = SessionMessages::new(config.clone());

        let empty_messages = vec![];
        let tokens_left = session.tokens_left_before_ejection(empty_messages);

        // With no messages, should have almost all tokens available
        // (minus preamble which is empty)
        let expected_max = (config.context_max_tokens as isize)
            - (config.assistant_minimum_context_tokens as isize);
        assert_eq!(tokens_left, expected_max);
    }

    #[test]
    fn test_serialize_chat_message_structure() {
        let config = create_test_config();
        let session = SessionMessages::new(config.clone());

        // Create a conversation first (this would normally be done via ensure_conversation_and_config)
        let conversation = Conversation {
            id: Some(1),
            session_name: "test".to_string(),
        };

        let message = SessionMessages::serialize_chat_message(
            "user".to_string(),
            "Test content".to_string(),
            true,
            &conversation,
        );

        assert_eq!(message.role, "user");
        assert_eq!(message.content, "Test content");
        assert_eq!(message.dynamic, true);
        assert_eq!(message.conversation_id, Some(1));
    }
}
