//! # Models Module
//!
//! This module defines the database models for the application.
//!
//! These models are tightly integrated with Diesel ORM to support operations
//! like querying, inserting, updating, and associating records in an SQLite database.
//!
//! # Overview
//!
//! - `AwfulConfig` stores API and session configuration settings.
//! - `Conversation` represents a named conversation session.
//! - `Message` records individual messages exchanged during conversations.
//!
//! All models derive traits like `Queryable`, `Insertable`, `Associations`, and `Selectable`
//! to allow efficient database interaction. They also implement `Clone` and `Debug` where appropriate.

use diesel::prelude::*;

/// Represents the stored configuration settings for a session.
///
/// Each `AwfulConfig` belongs to a specific `Conversation`, allowing
/// session-specific configuration overrides.
///
/// # Database
/// - Table: `awful_configs`
///
/// # Diesel Derivations
/// - `Queryable`, `Insertable`, `Associations`
///
/// # Fields
/// - `id`: Unique identifier (optional, auto-incremented).
/// - `api_base`: Base URL of the API.
/// - `api_key`: Authentication key for API access.
/// - `model`: Model name to be used for generation.
/// - `context_max_tokens`: Maximum allowed tokens for context.
/// - `assistant_minimum_context_tokens`: Reserved tokens for assistant's response.
/// - `stop_words`: Serialized string of stop words used in completions.
/// - `conversation_id`: Foreign key reference to the associated `Conversation`.
#[derive(Queryable, Associations, Insertable, PartialEq, Debug)]
#[diesel(belongs_to(Conversation))]
#[diesel(table_name = crate::schema::awful_configs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AwfulConfig {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub context_max_tokens: i32,
    pub assistant_minimum_context_tokens: i32,
    pub stop_words: String,
    pub conversation_id: Option<i32>,
}

/// Represents a named conversation between the user and assistant.
///
/// Conversations organize messages under distinct session names, enabling
/// saving and resuming multi-turn chats.
///
/// # Database
/// - Table: `conversations`
///
/// # Diesel Derivations
/// - `Queryable`, `Identifiable`, `Insertable`, `Selectable`
///
/// # Fields
/// - `id`: Unique identifier (optional, auto-incremented).
/// - `session_name`: Unique name assigned to the conversation.
#[derive(Queryable, Identifiable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Conversation {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub session_name: String,
}

/// Represents a single message exchanged in a conversation.
///
/// Messages are associated with a conversation and track both user
/// and assistant messages, including system prompts and dynamic generation.
///
/// # Database
/// - Table: `messages`
///
/// # Diesel Derivations
/// - `Queryable`, `Associations`, `Insertable`, `Selectable`, `Clone`
///
/// # Fields
/// - `id`: Unique identifier (optional, auto-incremented).
/// - `role`: Sender's role (`system`, `user`, `assistant`).
/// - `content`: Text content of the message.
/// - `dynamic`: Indicates if the message was dynamically generated.
/// - `conversation_id`: Foreign key reference to the associated `Conversation`.
#[derive(Queryable, Associations, Insertable, Debug, Selectable,  Clone)]
#[diesel(belongs_to(Conversation))]
#[diesel(table_name = crate::schema::messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Message {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub role: String,
    pub content: String,
    pub dynamic: bool,
    pub conversation_id: Option<i32>
}