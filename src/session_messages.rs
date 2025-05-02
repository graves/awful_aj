//! # Session Messages Module
//!
//! This module manages the lifecycle of chat session messages, including
//! persistence to a SQLite database, token counting, message serialization,
//! and managing session memory limits.
//!
//! It provides functionality for:
//! - Serializing and deserializing chat messages
//! - Inserting and querying conversations and messages
//! - Counting tokens to determine when old messages should be ejected
//! - Interfacing with both `async-openai` and database models
//! 
use async_openai::types::{ChatCompletionRequestMessage, Role};
use diesel::{Connection, SqliteConnection};

use crate::{
    config::{establish_connection, AwfulJadeConfig},
    models::{Conversation, Message},
};

use diesel::prelude::*;
use tiktoken_rs::cl100k_base;

/// Represents the session's messages, including both the system preamble
/// and the ongoing conversation.
///
/// Also holds a live database connection for persisting messages.
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
    /// Creates a new `SessionMessages` instance tied to a configuration.
    ///
    /// # Parameters
    /// - `config: AwfulJadeConfig`: Configuration for the session.
    ///
    /// # Returns
    /// - `Self`: New `SessionMessages` instance.
    pub fn new(config: AwfulJadeConfig) -> Self {
        Self {
            preamble_messages: Vec::new(),
            conversation_messages: Vec::new(),
            config: config.clone(),
            sqlite_connection: establish_connection(&config.session_db_url),
        }
    }

    /// Serializes a chat message into a `Message` database model.
    ///
    /// # Parameters
    /// - `role: String`: Role of the message (e.g., "user", "assistant").
    /// - `content: String`: Message content.
    /// - `dynamic: bool`: Whether the message is dynamic.
    /// - `conversation: &Conversation`: Conversation to associate with.
    ///
    /// # Returns
    /// - `Message`: Serialized message struct.
    pub fn serialize_chat_message(
        role: String,
        content: String,
        dynamic: bool,
        conversation: &Conversation,
    ) -> Message {
        Message {
            id: None,
            role: role,
            content: content,
            dynamic: dynamic,
            conversation_id: Some(conversation.id.unwrap()),
        }
    }

    /// Converts a `Role` and content into an OpenAI `ChatCompletionRequestMessage`.
    ///
    /// # Parameters
    /// - `role: Role`: Sender's role.
    /// - `content: String`: Content of the message.
    ///
    /// # Returns
    /// - `ChatCompletionRequestMessage`: New chat message.
    pub fn serialize_chat_completion_message(
        role: Role,
        content: String,
    ) -> ChatCompletionRequestMessage {
        ChatCompletionRequestMessage {
            role: role,
            content: Some(content.clone()),
            name: None,
            function_call: None,
        }
    }

    /// Persists a `Message` into the database.
    ///
    /// # Parameters
    /// - `message: &Message`: Message to persist.
    ///
    /// # Returns
    /// - `Result<Message, diesel::result::Error>`: Inserted message or error.
    pub fn persist_message(&mut self, message: &Message) -> Result<Message, diesel::result::Error> {
        let message: Message = self.sqlite_connection.transaction(|conn| {
            diesel::insert_into(crate::schema::messages::table)
                .values(message)
                .returning(Message::as_returning())
                .get_result(conn)
        })?;

        Ok(message)
    }

    /// Persists multiple `ChatCompletionRequestMessage` entries into the database.
    ///
    /// # Parameters
    /// - `messages: &Vec<ChatCompletionRequestMessage>`: Messages to persist.
    ///
    /// # Returns
    /// - `Result<Vec<Message>, diesel::result::Error>`: Inserted messages or error.
    pub fn persist_chat_completion_messages(
        &mut self,
        messages: &Vec<ChatCompletionRequestMessage>,
    ) -> Result<Vec<Message>, diesel::result::Error> {
        let mut persisted_messages = Vec::new();
        let conversation = self.query_conversation().unwrap();
        for message in messages {
            let content = message.content.as_ref().unwrap();
            let chat_message = Self::serialize_chat_message(
                message.role.to_string(),
                content.to_string(),
                false,
                &conversation,
            );
            let ret = self.persist_message(&chat_message);
            persisted_messages.push(ret.unwrap());
        }

        Ok(persisted_messages)
    }

    /// Inserts a new chat message into the database for the current conversation.
    ///
    /// # Parameters
    /// - `role: String`: Sender's role.
    /// - `content: String`: Content of the message.
    ///
    /// # Returns
    /// - `Result<Message, diesel::result::Error>`: Inserted message or error.
    pub fn insert_message(&mut self, role: String, content: String) -> Result<Message, diesel::result::Error> {
        let conversation = self.query_conversation();

        match conversation {
            Ok(convo) => {
                let chat_message = Self::serialize_chat_message(
                    role,
                    content,
                    false,
                    &convo,
                );
        
                return self.persist_message(&chat_message);
            },
            Err(err) =>  return Err(err)
        }
    }

    /// Queries the database for the current session's conversation.
    ///
    /// # Returns
    /// - `Result<Conversation, diesel::result::Error>`: Conversation or error.
    pub fn query_conversation(&mut self) -> Result<Conversation, diesel::result::Error> {
        let a_session_name = self
            .config
            .session_name
            .as_ref();

        if a_session_name.is_none() {
            return Err(diesel::result::Error::NotFound)
        }

        let conversation: Result<Conversation, diesel::result::Error> =
            self.sqlite_connection.transaction(|conn| {
                let existing_conversation: Result<Conversation, diesel::result::Error> =
                    crate::schema::conversations::table
                        .filter(crate::schema::conversations::session_name.eq(a_session_name.unwrap()))
                        .first(conn);

                existing_conversation
            });

        conversation
    }

    /// Queries the database for messages in a given conversation.
    ///
    /// # Parameters
    /// - `conversation: &Conversation`: Conversation to query.
    ///
    /// # Returns
    /// - `Result<Vec<Message>, diesel::result::Error>`: List of messages or error.
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

    /// Converts a `String` into an OpenAI `Role`.
    ///
    /// # Panics
    /// Panics if an unknown role string is encountered.
    ///
    /// # Parameters
    /// - `role: &String`: Role as a string.
    ///
    /// # Returns
    /// - `Role`: Parsed role.
    pub fn string_to_role(role: &String) -> Role {
        match role.as_str() {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            err => panic!("Role in message not allowed: {}", err),
        }
    }

    /// Counts the number of tokens in a `Message`.
    ///
    /// # Parameters
    /// - `message: &Message`: Message to tokenize.
    ///
    /// # Returns
    /// - `isize`: Number of tokens.
    pub fn count_tokens_in_message(message: &Message) -> isize {
        let bpe = cl100k_base().unwrap();
        let msg_tokens = bpe.encode_with_special_tokens(&message.content);

        msg_tokens.len() as isize
    }

    /// Counts the number of tokens across multiple `ChatCompletionRequestMessage` entries.
    ///
    /// # Parameters
    /// - `messages: &Vec<ChatCompletionRequestMessage>`: Messages to tokenize.
    ///
    /// # Returns
    /// - `isize`: Total number of tokens.
    pub fn count_tokens_in_chat_completion_messages(
        messages: &Vec<ChatCompletionRequestMessage>,
    ) -> isize {
        let bpe = cl100k_base().unwrap();
        let mut count: isize = 0;
        for msg in messages {
            if let Some(content) = &msg.content {
                let msg_tokens = bpe.encode_with_special_tokens(content);
                count += msg_tokens.len() as isize;
            }
        }

        count
    }

    /// Calculates how many tokens are available before messages must be ejected.
    ///
    /// # Parameters
    /// - `messages: Vec<Message>`: Messages in the conversation.
    ///
    /// # Returns
    /// - `isize`: Remaining token allowance.
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

    /// Returns the maximum allowed token budget for a session.
    ///
    /// # Returns
    /// - `isize`: Maximum number of tokens allowed before assistant cut-off.
    pub fn max_tokens(&self) -> isize {
        ((self.config.context_max_tokens as i32) - self.config.assistant_minimum_context_tokens)
            as isize
    }

    /// Determines if messages should be ejected based on current token usage.
    ///
    /// # Returns
    /// - `bool`: `true` if messages need to be ejected, otherwise `false`.
    pub fn should_eject_message(&self) -> bool {
        let session_token_count =
            Self::count_tokens_in_chat_completion_messages(&self.preamble_messages)
                + Self::count_tokens_in_chat_completion_messages(&self.conversation_messages);
        tracing::info!("SESSION TOKEN COUNT: {}", session_token_count);
        tracing::info!("ALLOTTED TOKENS {}", self.max_tokens());

        session_token_count > self.max_tokens()
    }
}
