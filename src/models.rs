//! # Database ORM Models for Awful Jade
//!
//! This module defines Diesel ORM models that map to SQLite tables for persisting
//! conversations, messages, and configuration snapshots. These models provide the
//! foundation for Awful Jade's session management and conversation history features.
//!
//! ## Overview
//!
//! The database schema consists of three main tables:
//!
//! | Table | Model | Purpose |
//! |-------|-------|---------|
//! | `conversations` | [`Conversation`] | Named chat sessions |
//! | `messages` | [`Message`] | Individual turns (system/user/assistant) |
//! | `awful_configs` | [`AwfulConfig`] | Configuration snapshots |
//!
//! ## Data Model Relationships
//!
//! ```text
//! Conversation (1) ─┬─ (*) Message
//!                   └─ (*) AwfulConfig
//! ```
//!
//! - **One conversation** has **many messages** (one per turn in the chat)
//! - **One conversation** has **many config snapshots** (one per configuration change)
//!
//! ## Core Models
//!
//! ### [`Conversation`]
//!
//! Represents a named chat session (e.g., `"default"`, `"project-refactor"`).
//! Multiple CLI invocations with the same session name share the same conversation,
//! enabling context continuity.
//!
//! ### [`Message`]
//!
//! Represents a single turn in a conversation. Messages have a `role` field:
//!
//! - `"system"`: System prompts and preamble
//! - `"user"`: User input
//! - `"assistant"`: Model responses
//!
//! ### [`AwfulConfig`]
//!
//! Point-in-time snapshot of configuration settings (API endpoint, model, token limits).
//! Used for auditing which settings were active during a conversation.
//!
//! ## Diesel Integration
//!
//! All models derive the appropriate Diesel traits:
//!
//! - **`Queryable`**: Load rows from database queries
//! - **`Insertable`**: Insert new rows
//! - **`Associations`**: Define foreign key relationships
//! - **`Identifiable`**: Enable `find()` by primary key
//! - **`Selectable`**: Type-safe column selection
//!
//! See [`crate::schema`] for the auto-generated table schemas.
//!
//! ## Usage Examples
//!
//! ### Creating a Conversation and Messages
//!
//! ```no_run
//! use diesel::prelude::*;
//! use awful_aj::schema::{conversations, messages};
//! use awful_aj::models::{Conversation, Message};
//!
//! # fn example(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new conversation
//! let conversation = diesel::insert_into(conversations::table)
//!     .values(&Conversation {
//!         id: None,
//!         session_name: "my-research".into(),
//!     })
//!     .returning(Conversation::as_returning())
//!     .get_result(conn)?;
//!
//! // Add a user message
//! let user_msg = diesel::insert_into(messages::table)
//!     .values(&Message {
//!         id: None,
//!         role: "user".into(),
//!         content: "What is HNSW?".into(),
//!         dynamic: true,
//!         conversation_id: conversation.id,
//!     })
//!     .returning(Message::as_returning())
//!     .get_result(conn)?;
//!
//! // Add an assistant response
//! let assistant_msg = diesel::insert_into(messages::table)
//!     .values(&Message {
//!         id: None,
//!         role: "assistant".into(),
//!         content: "HNSW (Hierarchical Navigable Small World) is...".into(),
//!         dynamic: true,
//!         conversation_id: conversation.id,
//!     })
//!     .returning(Message::as_returning())
//!     .get_result(conn)?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Querying Messages for a Conversation
//!
//! ```no_run
//! use diesel::prelude::*;
//! use awful_aj::schema::{conversations, messages};
//! use awful_aj::models::{Conversation, Message};
//!
//! # fn example(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
//! // Find conversation by session name
//! let conversation = conversations::table
//!     .filter(conversations::session_name.eq("my-research"))
//!     .first::<Conversation>(conn)?;
//!
//! // Load all messages for this conversation
//! let msgs = messages::table
//!     .filter(messages::conversation_id.eq(conversation.id))
//!     .select(Message::as_select())
//!     .load(conn)?;
//!
//! for msg in msgs {
//!     println!("{}: {}", msg.role, msg.content);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Storing Configuration Snapshots
//!
//! ```no_run
//! use diesel::prelude::*;
//! use awful_aj::schema::{conversations, awful_configs};
//! use awful_aj::models::{Conversation, AwfulConfig};
//!
//! # fn example(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
//! let conversation = conversations::table
//!     .filter(conversations::session_name.eq("my-research"))
//!     .first::<Conversation>(conn)?;
//!
//! // Store config snapshot
//! diesel::insert_into(awful_configs::table)
//!     .values(&AwfulConfig {
//!         id: None,
//!         conversation_id: conversation.id,
//!         api_base: "http://localhost:5001/v1".into(),
//!         api_key: "".into(),
//!         model: "qwen2.5-7b".into(),
//!         context_max_tokens: 8192,
//!         assistant_minimum_context_tokens: 2048,
//!         stop_words: "<|im_end|>,<|im_start|>".into(),
//!     })
//!     .execute(conn)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Database Schema
//!
//! See [`crate::schema`] for the complete table definitions generated by
//! `diesel print-schema`.
//!
//! ## See Also
//!
//! - [`crate::config::AwfulJadeConfig`] - In-memory configuration (maps to `AwfulConfig`)
//! - [`crate::session_messages::SessionMessages`] - High-level session management
//! - [`crate::schema`] - Auto-generated Diesel schema
use diesel::prelude::*;

/// Snapshot of runtime settings linked to a [`Conversation`].
///
/// An `AwfulConfig` row captures the **API** and **token budgeting** knobs that
/// were in effect for a given session at a given time. The app may insert a new
/// snapshot whenever those values change (see
/// [`crate::config::AwfulJadeConfig::ensure_conversation_and_config`]).
///
/// ### Table
/// - `awful_configs`
///
/// ### Associations
/// - `belongs_to(Conversation)`
///
/// ### Notes
/// - `stop_words` is stored as a single **comma-joined** string in the DB; the
///   higher-level config loads a `Vec<String>` from YAML and handles the join/split.
/// - `id` is optional for `Insertable` convenience; Diesel assigns it on insert.
#[derive(Queryable, Associations, Insertable, PartialEq, Debug)]
#[diesel(belongs_to(Conversation))]
#[diesel(table_name = crate::schema::awful_configs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AwfulConfig {
    /// Auto-increment primary key (set by the DB on insert).
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    /// Base URL of the OpenAI-compatible endpoint (e.g. `http://localhost:5001/v1`).
    pub api_base: String,
    /// API key/token; may be empty when talking to a local, unsecured backend.
    pub api_key: String,
    /// Model identifier to request from the backend.
    pub model: String,
    /// Maximum tokens for the assistant’s response (DB as `i32`).
    pub context_max_tokens: i32,
    /// Minimum tokens to keep budgeted for the assistant (DB as `i32`).
    pub assistant_minimum_context_tokens: i32,
    /// Comma-joined list of stop strings.
    pub stop_words: String,
    /// Foreign key to the owning [`Conversation`].
    pub conversation_id: Option<i32>,
}

/// A named chat session.
///
/// Conversations group messages and configuration snapshots under a human-readable
/// `session_name` (e.g., `default`, `research-notes`, `demo-2025-08-29`).
///
/// ### Table
/// - `conversations`
///
/// ### Derives
/// - `Identifiable` so you can `load`/`find` by primary key
/// - `Selectable` for returning typed rows in queries
#[derive(Queryable, Identifiable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Conversation {
    /// Auto-increment primary key (set by the DB on insert).
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    /// Unique session name for this conversation.
    pub session_name: String,
}

impl Conversation {
    /// Convenience accessor for the assigned primary key.
    ///
    /// Returns `Some(id)` once the row has been inserted.
    #[inline]
    pub fn id(&self) -> Option<i32> {
        self.id
    }
}

/// One turn in a conversation.
///
/// A `Message` represents either a system, user, or assistant utterance. It is
/// associated with exactly one [`Conversation`].
///
/// ### Table
/// - `messages`
///
/// ### Role values
/// - `"system"`: system instructions or preamble
/// - `"user"`: user input
/// - `"assistant"`: model output
///
/// ### Notes
/// - `dynamic` can be used to mark messages generated at runtime versus
///   static template rows.
/// - For convenience, this struct derives `Clone` to allow re-queuing
///   and buffering in memory before persistence.
#[derive(Queryable, Associations, Insertable, Debug, Selectable, Clone)]
#[diesel(belongs_to(Conversation))]
#[diesel(table_name = crate::schema::messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Message {
    /// Auto-increment primary key (set by the DB on insert).
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    /// Sender role: `"system"`, `"user"`, or `"assistant"`.
    pub role: String,
    /// Raw message text.
    pub content: String,
    /// `true` if generated dynamically (e.g., fetched/streamed), `false` if static.
    pub dynamic: bool,
    /// Foreign key to the owning [`Conversation`].
    pub conversation_id: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_creation() {
        let conversation = Conversation {
            id: None,
            session_name: "test_session".to_string(),
        };

        assert!(conversation.id.is_none());
        assert_eq!(conversation.session_name, "test_session");
    }

    #[test]
    fn test_conversation_with_id() {
        let conversation = Conversation {
            id: Some(42),
            session_name: "my_session".to_string(),
        };

        assert_eq!(conversation.id(), Some(42));
        assert_eq!(conversation.session_name, "my_session");
    }

    #[test]
    fn test_conversation_id_accessor() {
        let mut conversation = Conversation {
            id: None,
            session_name: "test".to_string(),
        };

        assert_eq!(conversation.id(), None);

        conversation.id = Some(123);
        assert_eq!(conversation.id(), Some(123));
    }

    #[test]
    fn test_message_creation() {
        let message = Message {
            id: None,
            role: "user".to_string(),
            content: "Hello world".to_string(),
            dynamic: true,
            conversation_id: Some(1),
        };

        assert!(message.id.is_none());
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "Hello world");
        assert!(message.dynamic);
        assert_eq!(message.conversation_id, Some(1));
    }

    #[test]
    fn test_message_roles() {
        let system_msg = Message {
            id: None,
            role: "system".to_string(),
            content: "System prompt".to_string(),
            dynamic: false,
            conversation_id: Some(1),
        };

        let user_msg = Message {
            id: None,
            role: "user".to_string(),
            content: "User query".to_string(),
            dynamic: true,
            conversation_id: Some(1),
        };

        let assistant_msg = Message {
            id: None,
            role: "assistant".to_string(),
            content: "Assistant response".to_string(),
            dynamic: true,
            conversation_id: Some(1),
        };

        assert_eq!(system_msg.role, "system");
        assert_eq!(user_msg.role, "user");
        assert_eq!(assistant_msg.role, "assistant");
    }

    #[test]
    fn test_message_dynamic_flag() {
        let static_message = Message {
            id: None,
            role: "system".to_string(),
            content: "Static content".to_string(),
            dynamic: false,
            conversation_id: None,
        };

        let dynamic_message = Message {
            id: None,
            role: "user".to_string(),
            content: "Dynamic content".to_string(),
            dynamic: true,
            conversation_id: None,
        };

        assert!(!static_message.dynamic);
        assert!(dynamic_message.dynamic);
    }

    #[test]
    fn test_message_clone() {
        let original = Message {
            id: Some(42),
            role: "user".to_string(),
            content: "Original message".to_string(),
            dynamic: true,
            conversation_id: Some(1),
        };

        let cloned = original.clone();

        assert_eq!(original.id, cloned.id);
        assert_eq!(original.role, cloned.role);
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.dynamic, cloned.dynamic);
        assert_eq!(original.conversation_id, cloned.conversation_id);
    }

    #[test]
    fn test_awful_config_creation() {
        let config = AwfulConfig {
            id: None,
            conversation_id: Some(1),
            api_base: "http://localhost:5001/v1".to_string(),
            api_key: "test_key".to_string(),
            model: "test_model".to_string(),
            context_max_tokens: 8192,
            assistant_minimum_context_tokens: 2048,
            stop_words: "<|im_end|>,<|im_start|>".to_string(),
        };

        assert!(config.id.is_none());
        assert_eq!(config.conversation_id, Some(1));
        assert_eq!(config.api_base, "http://localhost:5001/v1");
        assert_eq!(config.model, "test_model");
        assert_eq!(config.context_max_tokens, 8192);
        assert_eq!(config.assistant_minimum_context_tokens, 2048);
    }

    #[test]
    fn test_awful_config_stop_words_format() {
        let config = AwfulConfig {
            id: None,
            conversation_id: None,
            api_base: "http://localhost:5001/v1".to_string(),
            api_key: "".to_string(),
            model: "model".to_string(),
            context_max_tokens: 4096,
            assistant_minimum_context_tokens: 1024,
            stop_words: "word1,word2,word3".to_string(),
        };

        // Stop words are stored as comma-separated string
        assert!(config.stop_words.contains(","));
        assert_eq!(config.stop_words, "word1,word2,word3");
    }

    #[test]
    fn test_message_without_conversation() {
        let message = Message {
            id: None,
            role: "system".to_string(),
            content: "Standalone message".to_string(),
            dynamic: false,
            conversation_id: None,
        };

        assert_eq!(message.conversation_id, None);
    }
}
