//! # Brain module (working memory & preamble builder)
//!
//! The **brain** is a tiny in-process memory buffer that holds recent conversation
//! snippets (as [`Memory`] items) and helps build the *preamble* messages you pass
//! to the chat model. It’s intentionally simple:
//!
//! - Stores a queue (`VecDeque`) of memories with roles and content.
//! - Enforces a token budget (`max_tokens`) by evicting the oldest items.
//! - Produces *preamble* messages that include:
//!   - the template’s system prompt,
//!   - a serialized “brain state” (as JSON for the model to acknowledge),
//!   - an assistant “Ok” acknowledgement.
//!
//! The module doesn’t do embedding, ranking, or I/O — higher level code
//! (e.g., your vector store) can choose which memories to push in.
//!
//! ## Quick start
//! ```rust
//! use awful_aj::brain::{Brain, Memory};
//! use awful_aj::template::ChatTemplate;
//! use async_openai::types::{Role, ChatCompletionRequestMessage};
//! use awful_aj::session_messages::SessionMessages;
//!
//! // A tiny template:
//! let template = ChatTemplate {
//!     system_prompt: "You are Awful Jade, a helpful assistant.".to_string(),
//!     messages: vec![],
//!     response_format: None,
//!     pre_user_message_content: None,
//!     post_user_message_content: None,
//! };
//!
//! // Keep up to ~256 tokens of “working memory”:
//! let mut brain = Brain::new(256, &template);
//!
//! // SessionMessages integrates with the rest of the pipeline:
//! let cfg = awful_aj::config::AwfulJadeConfig {
//!     api_key: "".into(), api_base: "".into(), model: "".into(),
//!     context_max_tokens: 2048, assistant_minimum_context_tokens: 256,
//!     stop_words: vec![], session_db_url: "".into(),
//!     session_name: None, should_stream: None
//! };
//! let mut sess = SessionMessages::new(cfg);
//!
//! // Add a couple of memories:
//! brain.add_memory(Memory::new(Role::User, "Hello!".into()), &mut sess);
//! brain.add_memory(Memory::new(Role::Assistant, "Hi there 👋".into()), &mut sess);
//!
//! // Build the preamble that you prepend to a chat request:
//! let preamble: Vec<ChatCompletionRequestMessage> = brain.build_preamble().unwrap();
//! assert!(preamble.len() >= 3);
//! ```
//!
//! ## Notes on token limiting
//! - The token counting uses `tiktoken_rs::cl100k_base` with special tokens.
//! - If the budget is exceeded, the **oldest** memory is removed and the preamble
//!   is rebuilt on each eviction.
//! - **Caveat:** the current implementation computes `brain_token_count` once;
//!   if many evictions are required, you may want to recompute inside the loop.
//!   (Left as-is here to avoid changing behavior, but worth considering.)

use async_openai::types::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
};
use async_openai::types::{ChatCompletionRequestMessage, Role};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::collections::VecDeque;
use tiktoken_rs::cl100k_base;

use crate::session_messages::SessionMessages;
use crate::template::ChatTemplate;

/// A single conversational memory item (role + content).
///
/// This is the fundamental unit the brain stores. It’s deliberately small and serializable,
/// so you can persist/restore or shuttle memories between components.
///
/// # Examples
/// ```rust
/// use awful_aj::brain::Memory;
/// use async_openai::types::Role;
///
/// let m = Memory::new(Role::User, "Hello, world!".to_string());
/// assert_eq!(m.role, Role::User);
/// assert_eq!(m.content, "Hello, world!");
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Memory {
    /// The role of the message (User / Assistant / System).
    pub role: Role,
    /// The textual content of the message.
    pub content: String,
}

impl Memory {
    /// Construct a new [`Memory`].
    ///
    /// # Parameters
    /// - `role`: The chat role (e.g., [`Role::User`]).
    /// - `content`: The raw string content.
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }

    /// Serialize this memory into a small JSON object:
    /// `{"role": <role>, "content": <string>}`.
    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "role": self.role,
            "content": self.content,
        })
    }

    /// Deserialize a memory from JSON. Not used by default, but handy for tests or tooling.
    pub fn _from_json(json: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}

/// In-process working memory and preamble builder.
///
/// The `Brain` holds a queue of [`Memory`] entries and a maximum token budget. When you add a
/// new memory via [`Brain::add_memory`], it checks total token usage and evicts the oldest
/// entries until the budget is met. The **preamble** produced by [`Brain::build_preamble`]
/// starts each request with:
///
/// 1. A system message = your template’s `system_prompt`.
/// 2. A user message containing a JSON representation of the brain.
/// 3. An assistant message with `"Ok"` to acknowledge.
///
/// This “handshake” primes the model with context and a stable shape.
///
#[derive(Debug)]
pub struct Brain<'a> {
    /// FIFO store of memories (oldest at the front).
    pub memories: VecDeque<Memory>,
    /// Token budget enforced against the serialized brain JSON (see caveat above).
    pub max_tokens: u16,
    /// Reference to the chat template supplying the system prompt.
    pub template: &'a ChatTemplate,
}

impl<'a> Brain<'a> {
    /// Create a new brain with the given token budget and template reference.
    ///
    /// The brain does not own the template; you must keep it alive while using the brain.
    ///
    /// # Examples
    /// ```rust
    /// # use awful_aj::brain::Brain;
    /// # use awful_aj::template::ChatTemplate;
    /// let tpl = ChatTemplate {
    ///     system_prompt: "Be helpful".into(),
    ///     messages: vec![],
    ///     response_format: None,
    ///     pre_user_message_content: None,
    ///     post_user_message_content: None,
    /// };
    /// let brain = Brain::new(512, &tpl);
    /// assert_eq!(brain.max_tokens, 512);
    /// ```
    pub fn new(max_tokens: u16, template: &'a ChatTemplate) -> Self {
        Self {
            memories: VecDeque::<Memory>::new(),
            max_tokens,
            template,
        }
    }

    /// Push a memory and enforce the token limit.
    ///
    /// If the budget is exceeded, the oldest memory is removed repeatedly until the
    /// serialized brain fits within `max_tokens`. On each eviction, this refreshes
    /// the preamble in the provided [`SessionMessages`].
    ///
    /// # Parameters
    /// - `memory`: The new memory to append.
    /// - `session_messages`: Session scaffolding that holds preamble/conversation messages.
    ///
    /// # Behavior
    /// - Uses `cl100k_base()` BPE with special tokens when counting.
    /// - Rebuilds preamble on each eviction.
    pub fn add_memory(&mut self, memory: Memory, session_messages: &mut SessionMessages) {
        self.memories.push_back(memory);
        self.enforce_token_limit(session_messages);
    }

    /// Enforce `max_tokens` against the serialized brain JSON.
    ///
    /// When over budget, evicts from the **front** (oldest) and refreshes the preamble.
    ///
    /// ## Implementation note
    /// The current implementation computes `brain_token_count` once before the loop.
    /// If multiple evictions are needed, you may want to recompute inside the loop for
    /// stricter enforcement. Left untouched here to preserve existing behavior.
    fn enforce_token_limit(&mut self, session_messages: &mut SessionMessages) {
        tracing::info!("Enforcing token limit.");
        let bpe = cl100k_base().unwrap();
        let brain_token_count = bpe.encode_with_special_tokens(&self.get_serialized()).len();

        while brain_token_count > self.max_tokens as usize {
            tracing::info!("Brain token count is greater than {}", self.max_tokens);
            tracing::info!("Removing oldest memory");
            self.memories.remove(0); // Removing the oldest memory
            session_messages.preamble_messages = self.build_preamble().unwrap();
        }
    }

    /// Serialize the current brain as a compact JSON envelope the model can “acknowledge”.
    ///
    /// The output is a short explanatory preface followed by the JSON payload. Example:
    ///
    /// ```text
    /// Below is a JSON representation of our conversation leading up to this point...
    /// {"about":"This JSON object ...","memories":[{"role":"user","content":"..."}, ...]}
    /// ```
    ///
    /// This is primarily consumed by [`build_preamble`](Self::build_preamble).
    pub fn get_serialized(&self) -> String {
        let about = "This JSON object is a representation of our conversation leading up to this point. This object represents your memories.";

        let mut map = HashMap::new();
        map.insert("about", JsonValue::String(about.into()));
        map.insert(
            "memories",
            JsonValue::Array(self.memories.iter().map(|m| m.to_json()).collect()),
        );

        let body = "Below is a JSON representation of our conversation leading up to this point. Please only respond to this message with \"Ok.\":\n";

        format!(
            "{}{}",
            body,
            serde_json::to_string(&map).expect("Failed to serialize brain")
        )
    }

    /// Build the standard preamble: system → user(brain JSON) → assistant("Ok").
    ///
    /// This is the preamble used by most flows to ensure the assistant has a compact view
    /// of recent state before answering a new turn.
    ///
    /// # Errors
    /// Returns `Err(&'static str)` if building messages fails (unlikely under normal usage).
    #[allow(deprecated)]
    pub fn build_preamble(&self) -> Result<Vec<ChatCompletionRequestMessage>, &'static str> {
        let system_chat_completion =
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(
                    self.template.system_prompt.clone(),
                ),
                name: None,
            });

        let mut messages: Vec<ChatCompletionRequestMessage> = vec![system_chat_completion];

        let brain_json = self.get_serialized();
        tracing::info!("State of brain: {:?}", brain_json);

        let user_chat_completion =
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(brain_json),
                name: None,
            });

        messages.push(user_chat_completion);

        let assistant_chat_completion =
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                    "Ok".to_string(),
                )),
                name: None,
                refusal: None,
                audio: None,
                tool_calls: None,
                function_call: None,
            });

        messages.push(assistant_chat_completion);

        Ok(messages)
    }

    /// Build a “brainless” preamble (same shape, currently still includes `get_serialized()`).
    ///
    /// This variant keeps the same three-message structure as [`build_preamble`]. In the current
    /// implementation it still injects the brain JSON; callers who truly want a “no brain”
    /// preamble can supply an empty brain or post-filter the returned messages.
    ///
    /// # Errors
    /// Returns `Err(&'static str)` if building messages fails (unlikely under normal usage).
    #[allow(deprecated)]
    pub fn build_brainless_preamble(
        &self,
    ) -> Result<Vec<ChatCompletionRequestMessage>, &'static str> {
        let system_chat_completion =
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(
                    self.template.system_prompt.clone(),
                ),
                name: None,
            });

        let mut messages: Vec<ChatCompletionRequestMessage> = vec![system_chat_completion];

        let brain_json = self.get_serialized();
        tracing::info!("State of brain: {:?}", brain_json);

        let user_chat_completion =
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(brain_json),
                name: None,
            });

        messages.push(user_chat_completion);

        let assistant_chat_completion =
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                    "Ok".to_string(),
                )),
                name: None,
                refusal: None,
                audio: None,
                tool_calls: None,
                function_call: None,
            });

        messages.push(assistant_chat_completion);

        Ok(messages)
    }
}
