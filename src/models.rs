//! # Database models
//!
//! Data structures that map to the project’s SQLite schema via **Diesel**.
//!
//! These models are used by higher-level modules to persist and query:
//!
//! - [`AwfulConfig`]: point-in-time snapshots of runtime settings tied to a
//!   specific conversation.
//! - [`Conversation`]: a named chat thread (session).
//! - [`Message`]: one record per turn (system/user/assistant) within a conversation.
//!
//! ## Diesel expectations
//!
//! This module assumes the following tables exist (see `crate::schema` generated
//! by `diesel print-schema`):
//!
//! - `awful_configs`
//! - `conversations`
//! - `messages`
//!
//! Each struct derives the appropriate Diesel traits (`Queryable`, `Insertable`,
//! `Associations`, `Identifiable`, `Selectable`) and is annotated with
//! `#[diesel(table_name = ...)]` and `#[diesel(belongs_to(...))]` where needed.
//!
//! ## Basic usage
//!
/// ```no_run
/// use diesel::prelude::*;
/// use awful_aj::schema::{conversations, messages};
/// use awful_aj::models::{Conversation, Message};
///
/// # fn demo(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
/// let convo: Conversation = diesel::insert_into(conversations::table)
///     .values(&Conversation { id: None, session_name: "demo".into() })
///     .returning(Conversation::as_returning())
///     .get_result(conn)?;
///
/// let _msg: Message = diesel::insert_into(messages::table)
///     .values(&Message{ id: None, role: "user".into(), content: "Hi".into(), dynamic: false, conversation_id: convo.id })
///     .returning(Message::as_returning())
///     .get_result(conn)?;
/// # Ok(()) }
/// ```
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
