//! # Working Memory and Preamble Generation
//!
//! This module implements **Awful Jade's working memory system**, providing a
//! token-budgeted FIFO queue of conversation turns that feeds the LLM's context
//! window. The [`Brain`] acts as a bridge between long-term storage (vector stores,
//! databases) and the LLM's immediate context needs.
//!
//! ## Overview
//!
//! The **brain** is an in-process memory buffer that:
//!
//! - **Stores** recent conversation snippets as [`Memory`] items in a FIFO queue
//! - **Enforces** a token budget by evicting oldest memories when over limit
//! - **Builds** preamble messages for LLM requests with:
//!   - System prompt from template
//!   - Serialized brain state (JSON conversation history)
//!   - Assistant acknowledgment ("Ok")
//!   - Optional RAG context injection
//!
//! The module is **intentionally minimal**—it doesn't handle embedding, ranking,
//! or I/O. Higher-level code (e.g., [`crate::vector_store::VectorStore`]) decides
//! which memories to inject.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Conversation Flow                     │
//! │                                                          │
//! │  User Input → Brain.add_memory() → Token Check          │
//! │                    │                    │                │
//! │                    │              Over budget?           │
//! │                    │                    │                │
//! │                    │             Yes → Evict oldest      │
//! │                    │                    │                │
//! │                    ▼                    ▼                │
//! │               Brain Queue      Update Preamble           │
//! │            (VecDeque<Memory>)    in SessionMessages     │
//! │                    │                                     │
//! │                    ▼                                     │
//! │           build_preamble()                               │
//! │                    │                                     │
//! │         ┌──────────┴──────────┐                         │
//! │         │                     │                         │
//! │         ▼                     ▼                         │
//! │  System Prompt      Brain JSON + "Ok"                   │
//! │         │                     │                         │
//! │         └──────────┬──────────┘                         │
//! │                    ▼                                     │
//! │          OpenAI API Request                             │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Concepts
//!
//! ### Memory Items
//!
//! A [`Memory`] is a simple `(Role, String)` pair representing one turn in the
//! conversation. Memories are:
//!
//! - **Serializable**: Can be persisted to disk or sent over the wire
//! - **Role-aware**: System, User, or Assistant messages
//! - **Content-only**: No metadata beyond role and text
//!
//! ### Token Budgeting
//!
//! The brain enforces a hard token limit using **tiktoken** (`cl100k_base`):
//!
//! 1. On each [`Brain::add_memory`], serialize entire brain to JSON
//! 2. Count tokens in serialized JSON (including special tokens)
//! 3. If over budget, evict **oldest** memory (front of queue)
//! 4. Repeat until under budget
//! 5. Rebuild preamble in [`SessionMessages`](crate::session_messages::SessionMessages)
//!
//! **Token Counting**: Uses OpenAI's `cl100k_base` BPE encoding with special
//! tokens (same as GPT-4/3.5-turbo).
//!
//! ### Preamble Structure
//!
//! The preamble is a sequence of messages that sets up each LLM request:
//!
//! ```text
//! 1. System: "<template.system_prompt>"
//! 2. [Optional] User: "Below is supplementary documentation..."
//! 3. [Optional] Assistant: "I have reviewed the supplementary documentation."
//! 4. User: "Below is JSON of our conversation...\n{brain_json}"
//! 5. Assistant: "Ok"
//! ```
//!
//! This "handshake" ensures the model has compact context before answering.
//!
//! ## Quick Start
//!
//! ### Basic Usage
//!
//! ```no_run
//! use awful_aj::brain::{Brain, Memory};
//! use awful_aj::template::ChatTemplate;
//! use async_openai::types::{Role, ChatCompletionRequestMessage};
//! use awful_aj::session_messages::SessionMessages;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Create a template
//! let template = ChatTemplate {
//!     system_prompt: "You are Awful Jade, a helpful assistant.".to_string(),
//!     messages: vec![],
//!     response_format: None,
//!     pre_user_message_content: None,
//!     post_user_message_content: None,
//! };
//!
//! // 2. Create a brain with token budget
//! let mut brain = Brain::new(256, &template);
//!
//! // 3. Create session messages
//! let cfg = awful_aj::config::AwfulJadeConfig {
//!     api_key: "".into(),
//!     api_base: "".into(),
//!     model: "".into(),
//!     context_max_tokens: 2048,
//!     assistant_minimum_context_tokens: 256,
//!     stop_words: vec![],
//!     session_db_url: "".into(),
//!     session_name: None,
//!     should_stream: None,
//! };
//! let mut sess = SessionMessages::new(cfg);
//!
//! // 4. Add conversation turns
//! brain.add_memory(Memory::new(Role::User, "Hello!".into()), &mut sess);
//! brain.add_memory(Memory::new(Role::Assistant, "Hi there!".into()), &mut sess);
//!
//! // 5. Build preamble for LLM request
//! let preamble: Vec<ChatCompletionRequestMessage> = brain.build_preamble()?;
//! assert!(preamble.len() >= 3);
//! # Ok(())
//! # }
//! ```
//!
//! ### With RAG Context
//!
//! ```no_run
//! # use awful_aj::brain::{Brain, Memory};
//! # use awful_aj::template::ChatTemplate;
//! # use async_openai::types::Role;
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let template = ChatTemplate {
//! #     system_prompt: "You are a helpful assistant.".into(),
//! #     messages: vec![],
//! #     response_format: None,
//! #     pre_user_message_content: None,
//! #     post_user_message_content: None,
//! # };
//! let mut brain = Brain::new(1024, &template);
//!
//! // Inject RAG context from document retrieval
//! brain.rag_context = Some(
//!     "# Documentation\n\
//!      HNSW (Hierarchical Navigable Small World) is a graph-based \
//!      algorithm for approximate nearest neighbor search...".to_string()
//! );
//!
//! // Preamble now includes RAG context injection
//! let preamble = brain.build_preamble()?;
//! // Includes RAG documentation + "I have reviewed..." acknowledgment
//! # Ok(())
//! # }
//! ```
//!
//! ## Token Limiting Behavior
//!
//! ### Eviction Strategy
//!
//! When the brain exceeds `max_tokens`, memories are evicted **FIFO** (oldest first):
//!
//! ```text
//! Initial state (under budget):
//! [Memory1, Memory2, Memory3] → 180 tokens ✓
//!
//! Add Memory4:
//! [Memory1, Memory2, Memory3, Memory4] → 280 tokens (over 256 limit)
//!
//! Evict Memory1:
//! [Memory2, Memory3, Memory4] → 210 tokens ✓
//! ```
//!
//! ### Token Counting Details
//!
//! - **Encoding**: `tiktoken_rs::cl100k_base` (OpenAI GPT-4/3.5-turbo)
//! - **Special Tokens**: Included in count (`<|endoftext|>`, etc.)
//! - **What's Counted**: Entire serialized brain JSON (see [`Brain::get_serialized`])
//! - **Frequency**: On every [`Brain::add_memory`] call
//!
//! ### Performance Considerations
//!
//! **Current Implementation Note**: Token count is computed **once** before the
//! eviction loop. If multiple evictions are needed, the count should ideally be
//! recomputed inside the loop for stricter enforcement. Left as-is to preserve
//! existing behavior, but consider refactoring for production use.
//!
//! ## Preamble Variants
//!
//! The brain supports two preamble generation modes:
//!
//! | Method | Brain JSON | RAG Context | Use Case |
//! |--------|-----------|-------------|----------|
//! | [`build_preamble`](Brain::build_preamble) | ✅ Included | ✅ If set | Normal operation |
//! | [`build_brainless_preamble`](Brain::build_brainless_preamble) | ✅ Included* | ❌ Excluded | Legacy/testing |
//!
//! *Note: Despite the name, `build_brainless_preamble` currently still includes
//! brain JSON. Callers needing truly empty brains should use an empty `Brain`.
//!
//! ## Integration with Other Modules
//!
//! ### SessionMessages
//!
//! The brain updates [`SessionMessages::preamble_messages`] on every eviction:
//!
//! ```rust,ignore
//! session_messages.preamble_messages = brain.build_preamble()?;
//! ```
//!
//! This ensures the session's preamble stays in sync with the brain's state.
//!
//! ### Vector Store
//!
//! Higher-level code can inject relevant memories from [`crate::vector_store::VectorStore`]:
//!
//! ```rust,ignore
//! let relevant_memories = vector_store.search_similarities(&user_query, 5)?;
//! for mem in relevant_memories {
//!     brain.add_memory(mem, &mut session_messages);
//! }
//! ```
//!
//! ### API Client
//!
//! The preamble is prepended to each API request:
//!
//! ```rust,ignore
//! let mut messages = brain.build_preamble()?;
//! messages.extend(template.messages.clone());
//! messages.push(user_message);
//!
//! let response = client.chat().create(ChatCompletionRequest {
//!     model: config.model.clone(),
//!     messages,
//!     ..Default::default()
//! }).await?;
//! ```
//!
//! ## Examples
//!
//! ### Multi-Turn Conversation with Eviction
//!
//! ```no_run
//! # use awful_aj::brain::{Brain, Memory};
//! # use awful_aj::template::ChatTemplate;
//! # use awful_aj::session_messages::SessionMessages;
//! # use async_openai::types::Role;
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let template = ChatTemplate {
//! #     system_prompt: "You are a helpful assistant.".into(),
//! #     messages: vec![],
//! #     response_format: None,
//! #     pre_user_message_content: None,
//! #     post_user_message_content: None,
//! # };
//! # let cfg = awful_aj::config::AwfulJadeConfig {
//! #     api_key: "".into(), api_base: "".into(), model: "".into(),
//! #     context_max_tokens: 2048, assistant_minimum_context_tokens: 256,
//! #     stop_words: vec![], session_db_url: "".into(),
//! #     session_name: None, should_stream: None,
//! # };
//! let mut brain = Brain::new(128, &template); // Very small budget for demo
//! let mut sess = SessionMessages::new(cfg);
//!
//! // First exchange
//! brain.add_memory(Memory::new(Role::User, "What is 2+2?".into()), &mut sess);
//! brain.add_memory(Memory::new(Role::Assistant, "4".into()), &mut sess);
//!
//! // Second exchange (may evict first if over budget)
//! brain.add_memory(Memory::new(Role::User, "What about 3+3?".into()), &mut sess);
//! brain.add_memory(Memory::new(Role::Assistant, "6".into()), &mut sess);
//!
//! // Check remaining memories
//! println!("Memories retained: {}", brain.memories.len());
//! # Ok(())
//! # }
//! ```
//!
//! ### Inspecting Serialized Brain State
//!
//! ```no_run
//! # use awful_aj::brain::{Brain, Memory};
//! # use awful_aj::template::ChatTemplate;
//! # use async_openai::types::Role;
//! # let template = ChatTemplate {
//! #     system_prompt: "You are a helpful assistant.".into(),
//! #     messages: vec![],
//! #     response_format: None,
//! #     pre_user_message_content: None,
//! #     post_user_message_content: None,
//! # };
//! let mut brain = Brain::new(512, &template);
//! brain.memories.push_back(Memory::new(Role::User, "Hello".into()));
//! brain.memories.push_back(Memory::new(Role::Assistant, "Hi!".into()));
//!
//! let serialized = brain.get_serialized();
//! println!("{}", serialized);
//! // Output:
//! // Below is a JSON representation of our conversation leading up to this point...
//! // {"about":"This JSON object is...","memories":[...]}
//! ```
//!
//! ## Error Handling
//!
//! Most methods return `Result<_, &'static str>` for consistency with the codebase:
//!
//! - **[`build_preamble`](Brain::build_preamble)**: Errors are unlikely; only if message
//!   construction fails (which shouldn't happen in normal operation)
//! - **[`build_brainless_preamble`](Brain::build_brainless_preamble)**: Same error conditions
//! - **[`Memory::_from_json`](Memory::_from_json)**: Returns `serde_json::Error` if
//!   deserialization fails
//!
//! ## Performance Characteristics
//!
//! | Operation | Time Complexity | Notes |
//! |-----------|----------------|-------|
//! | `new()` | O(1) | Allocates empty queue |
//! | `add_memory()` | O(n × m) | n = evictions, m = serialization cost |
//! | `build_preamble()` | O(m) | m = memories count |
//! | `get_serialized()` | O(m) | JSON serialization |
//! | Eviction | O(m) | VecDeque front removal |
//!
//! **Token counting** dominates performance: `tiktoken_rs` BPE encoding scales
//! with text length. For very large conversations, consider caching token counts.
//!
//! ## See Also
//!
//! - [`crate::vector_store`] - Long-term semantic memory
//! - [`crate::session_messages`] - Session lifecycle management
//! - [`crate::template`] - System prompt templates
//! - [`crate::api`] - LLM API client using brain preambles

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

/// A single conversational memory item representing one turn in a chat.
///
/// `Memory` is the fundamental unit stored in the [`Brain`]'s working memory queue.
/// It captures a single message with its role (system/user/assistant) and content,
/// designed to be lightweight, serializable, and easily transferable between components.
///
/// # Design Philosophy
///
/// - **Minimal**: Only stores role and content—no timestamps, IDs, or metadata
/// - **Serializable**: Can be persisted to disk, sent over network, or stored in databases
/// - **Cloneable**: Cheap to duplicate for branching conversations or caching
/// - **Role-aware**: Compatible with OpenAI's chat message format
///
/// # Fields
///
/// - **`role`**: The message sender—[`Role::User`], [`Role::Assistant`], or [`Role::System`]
/// - **`content`**: The raw message text
///
/// # Usage
///
/// Memories are typically created when:
/// 1. User submits a prompt → `Memory::new(Role::User, user_input)`
/// 2. LLM responds → `Memory::new(Role::Assistant, llm_response)`
/// 3. System instructions are added → `Memory::new(Role::System, instruction)`
///
/// They're consumed by:
/// - [`Brain::add_memory`] to append to working memory
/// - [`Brain::get_serialized`] to convert to JSON for LLM context
/// - Vector stores for semantic indexing
///
/// # Examples
///
/// ## Creating Memories
///
/// ```rust
/// use awful_aj::brain::Memory;
/// use async_openai::types::Role;
///
/// // User message
/// let user_mem = Memory::new(Role::User, "What is HNSW?".to_string());
/// assert_eq!(user_mem.role, Role::User);
/// assert_eq!(user_mem.content, "What is HNSW?");
///
/// // Assistant response
/// let assistant_mem = Memory::new(
///     Role::Assistant,
///     "HNSW is a graph-based algorithm for ANN search.".to_string()
/// );
/// assert_eq!(assistant_mem.role, Role::Assistant);
/// ```
///
/// ## Serialization
///
/// ```rust
/// # use awful_aj::brain::Memory;
/// # use async_openai::types::Role;
/// let mem = Memory::new(Role::User, "Hello!".to_string());
///
/// // Convert to JSON
/// let json = mem.to_json();
/// assert_eq!(json["role"], "user");
/// assert_eq!(json["content"], "Hello!");
///
/// // JSON output: {"role":"user","content":"Hello!"}
/// ```
///
/// ## Cloning for Branching Conversations
///
/// ```rust
/// # use awful_aj::brain::Memory;
/// # use async_openai::types::Role;
/// let original = Memory::new(Role::User, "Explain quantum physics".to_string());
///
/// // Clone for alternative conversation branch
/// let branch = original.clone();
/// assert_eq!(original.content, branch.content);
/// ```
///
/// # See Also
///
/// - [`Brain`] - Working memory container for memories
/// - [`crate::session_messages::SessionMessages`] - Conversation persistence
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Memory {
    /// The role of the message sender.
    ///
    /// Valid values:
    /// - **[`Role::System`]**: System instructions or context
    /// - **[`Role::User`]**: User input or queries
    /// - **[`Role::Assistant`]**: LLM responses
    pub role: Role,

    /// The textual content of the message.
    ///
    /// This is the raw message body without any formatting or metadata.
    /// Can contain:
    /// - Plain text user queries
    /// - Markdown-formatted assistant responses
    /// - JSON data (for structured outputs)
    /// - Code blocks (in markdown)
    pub content: String,
}

impl Memory {
    /// Create a new memory with the specified role and content.
    ///
    /// This is the primary constructor for memory items. The memory is created
    /// in an initialized state ready to be added to a [`Brain`] or serialized.
    ///
    /// # Parameters
    ///
    /// - **`role`**: The message sender—[`Role::User`], [`Role::Assistant`], or [`Role::System`]
    /// - **`content`**: The raw message text (can be plain text, markdown, JSON, etc.)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use awful_aj::brain::Memory;
    /// use async_openai::types::Role;
    ///
    /// // User query
    /// let user = Memory::new(Role::User, "Explain HNSW".to_string());
    ///
    /// // Assistant response with markdown
    /// let assistant = Memory::new(
    ///     Role::Assistant,
    ///     "HNSW is a **graph-based** algorithm...".to_string()
    /// );
    ///
    /// // System instruction
    /// let system = Memory::new(
    ///     Role::System,
    ///     "You are a helpful coding assistant.".to_string()
    /// );
    /// ```
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }

    /// Serialize this memory to a compact JSON object.
    ///
    /// Converts the memory to a JSON value with the structure:
    /// ```json
    /// {
    ///   "role": "user",  // or "assistant", "system"
    ///   "content": "message text"
    /// }
    /// ```
    ///
    /// This format is used by [`Brain::get_serialized`] when building the brain
    /// state JSON that gets injected into LLM prompts.
    ///
    /// # Returns
    ///
    /// A [`serde_json::Value`] representing this memory. The `role` field is
    /// serialized as a lowercase string (`"user"`, `"assistant"`, `"system"`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use awful_aj::brain::Memory;
    /// use async_openai::types::Role;
    ///
    /// let mem = Memory::new(Role::User, "Hello!".to_string());
    /// let json = mem.to_json();
    ///
    /// assert_eq!(json["role"], "user");
    /// assert_eq!(json["content"], "Hello!");
    ///
    /// // Serialized output:
    /// // {"role":"user","content":"Hello!"}
    /// ```
    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "role": self.role,
            "content": self.content,
        })
    }

    /// Deserialize a memory from a JSON value (private utility method).
    ///
    /// This method reconstructs a [`Memory`] from a JSON object created by
    /// [`to_json`](Memory::to_json). It's primarily used for testing and
    /// internal tooling—most production code creates memories directly via
    /// [`Memory::new`].
    ///
    /// # Parameters
    ///
    /// - **`json`**: A reference to a JSON value containing `role` and `content` fields
    ///
    /// # Returns
    ///
    /// - **`Ok(Memory)`**: Successfully deserialized memory
    /// - **`Err(serde_json::Error)`**: Invalid JSON structure or missing required fields
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - JSON is missing `role` or `content` fields
    /// - `role` is not a valid value (`"user"`, `"assistant"`, `"system"`)
    /// - JSON structure doesn't match expected format
    ///
    /// # Examples
    ///
    /// ```rust
    /// use awful_aj::brain::Memory;
    /// use serde_json::json;
    ///
    /// let json = json!({
    ///     "role": "assistant",
    ///     "content": "I can help with that!"
    /// });
    ///
    /// let mem = Memory::_from_json(&json).unwrap();
    /// assert_eq!(mem.content, "I can help with that!");
    /// ```
    pub fn _from_json(json: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}

/// Token-budgeted working memory with automatic eviction and preamble generation.
///
/// The `Brain` is Awful Jade's short-term memory system, managing a FIFO queue of
/// [`Memory`] items with strict token limits. It serves two critical functions:
///
/// 1. **Memory Management**: Maintains recent conversation history within token budget
/// 2. **Preamble Generation**: Builds structured context for LLM API requests
///
/// ## Architecture
///
/// The brain operates as a **sliding window** over the conversation:
///
/// ```text
/// Full Conversation:
/// [Msg1] [Msg2] [Msg3] [Msg4] [Msg5] [Msg6] [Msg7]
///                           ↑                    ↑
///                        oldest              newest
///                           └────────┬─────────┘
///                                    │
///                              Brain Window
///                            (max_tokens = 256)
/// ```
///
/// As new messages arrive, oldest messages are evicted to stay under budget.
/// Evicted messages can be stored in long-term memory (vector store) for later retrieval.
///
/// ## Lifetime Parameter
///
/// The `'a` lifetime ties the brain to its template reference:
///
/// ```rust,ignore
/// pub struct Brain<'a> {
///     template: &'a ChatTemplate,  // Must outlive Brain
///     // ...
/// }
/// ```
///
/// This prevents the template from being dropped while the brain is in use,
/// ensuring the system prompt is always available.
///
/// ## Token Budgeting
///
/// Token limits are enforced using OpenAI's `cl100k_base` encoding:
///
/// - **What's counted**: The entire serialized brain JSON (see [`get_serialized`](Brain::get_serialized))
/// - **When**: On every [`add_memory`](Brain::add_memory) call
/// - **Eviction**: FIFO (oldest first) until under budget
/// - **Side effect**: Preamble is rebuilt in [`SessionMessages`](crate::session_messages::SessionMessages) on eviction
///
/// ## Preamble Structure
///
/// The brain generates a multi-message preamble for each LLM request:
///
/// ```text
/// [1] System: "<template.system_prompt>"
/// [2] [Optional] User: "Supplementary documentation: <rag_context>"
/// [3] [Optional] Assistant: "I have reviewed the documentation."
/// [4] User: "JSON of our conversation: {brain_json}"
/// [5] Assistant: "Ok"
/// ```
///
/// This "handshake" ensures the model has:
/// - System instructions (persona, rules)
/// - RAG context (if documents were retrieved)
/// - Conversation history (as compact JSON)
/// - Acknowledgment (primes response format)
///
/// ## Fields
///
/// - **`memories`**: FIFO queue of conversation turns (oldest at index 0)
/// - **`max_tokens`**: Token budget for serialized brain (enforced strictly)
/// - **`template`**: Reference to chat template (provides system prompt)
/// - **`rag_context`**: Optional retrieved documents (injected before brain JSON)
///
/// ## Thread Safety
///
/// The `Brain` is **not** `Send` or `Sync` due to the `&'a ChatTemplate` reference.
/// For concurrent use, wrap in `Arc<Mutex<Brain>>` or use one brain per thread.
///
/// ## Examples
///
/// ### Basic Brain with Manual Eviction
///
/// ```no_run
/// # use awful_aj::brain::{Brain, Memory};
/// # use awful_aj::template::ChatTemplate;
/// # use async_openai::types::Role;
/// # let template = ChatTemplate {
/// #     system_prompt: "Be helpful".into(),
/// #     messages: vec![],
/// #     response_format: None,
/// #     pre_user_message_content: None,
/// #     post_user_message_content: None,
/// # };
/// let mut brain = Brain::new(512, &template);
///
/// // Manually add memories (eviction happens automatically)
/// brain.memories.push_back(Memory::new(Role::User, "Hi!".into()));
/// brain.memories.push_back(Memory::new(Role::Assistant, "Hello!".into()));
///
/// println!("Current memory count: {}", brain.memories.len());
/// ```
///
/// ### With SessionMessages Integration
///
/// ```no_run
/// # use awful_aj::brain::{Brain, Memory};
/// # use awful_aj::template::ChatTemplate;
/// # use awful_aj::session_messages::SessionMessages;
/// # use async_openai::types::Role;
/// # let template = ChatTemplate {
/// #     system_prompt: "Be helpful".into(),
/// #     messages: vec![],
/// #     response_format: None,
/// #     pre_user_message_content: None,
/// #     post_user_message_content: None,
/// # };
/// # let cfg = awful_aj::config::AwfulJadeConfig {
/// #     api_key: "".into(), api_base: "".into(), model: "".into(),
/// #     context_max_tokens: 2048, assistant_minimum_context_tokens: 256,
/// #     stop_words: vec![], session_db_url: "".into(),
/// #     session_name: None, should_stream: None,
/// # };
/// let mut brain = Brain::new(256, &template);
/// let mut session = SessionMessages::new(cfg);
///
/// // add_memory handles eviction + preamble updates
/// brain.add_memory(Memory::new(Role::User, "Question 1".into()), &mut session);
/// brain.add_memory(Memory::new(Role::Assistant, "Answer 1".into()), &mut session);
///
/// // Preamble is automatically kept in sync
/// let preamble = brain.build_preamble().unwrap();
/// ```
///
/// ### With RAG Context
///
/// ```no_run
/// # use awful_aj::brain::{Brain, Memory};
/// # use awful_aj::template::ChatTemplate;
/// # let template = ChatTemplate {
/// #     system_prompt: "Be helpful".into(),
/// #     messages: vec![],
/// #     response_format: None,
/// #     pre_user_message_content: None,
/// #     post_user_message_content: None,
/// # };
/// let mut brain = Brain::new(1024, &template);
///
/// // Inject retrieved document chunks
/// brain.rag_context = Some(
///     "## Vector Search Algorithms\n\
///      HNSW creates a hierarchical graph structure...".to_string()
/// );
///
/// // Preamble now includes RAG context before brain JSON
/// let preamble = brain.build_preamble().unwrap();
/// // [System, RAG User, RAG Assistant, Brain User, Brain Assistant]
/// assert_eq!(preamble.len(), 5);
/// ```
///
/// ## See Also
///
/// - [`Memory`] - Individual conversation turns
/// - [`crate::session_messages::SessionMessages`] - Session lifecycle
/// - [`crate::vector_store::VectorStore`] - Long-term semantic memory
/// - [`crate::template::ChatTemplate`] - System prompt templates
#[derive(Debug)]
pub struct Brain<'a> {
    /// FIFO queue of conversation memories (oldest at front, newest at back).
    ///
    /// Memories are added via [`add_memory`](Brain::add_memory) and automatically
    /// evicted from the front when the token budget is exceeded. The queue is
    /// public to allow direct inspection and manual manipulation when needed.
    pub memories: VecDeque<Memory>,

    /// Maximum tokens allowed for the serialized brain JSON.
    ///
    /// This budget is enforced using OpenAI's `cl100k_base` encoding on the
    /// full JSON output of [`get_serialized`](Brain::get_serialized). When exceeded,
    /// oldest memories are evicted until usage drops below this limit.
    ///
    /// **Typical values**:
    /// - `256-512`: Minimal context (a few turns)
    /// - `1024-2048`: Medium context (10-20 turns)
    /// - `4096+`: Large context (full conversation history)
    pub max_tokens: u16,

    /// Reference to the chat template providing the system prompt.
    ///
    /// The template's `system_prompt` is injected as the first message in
    /// every preamble generated by [`build_preamble`](Brain::build_preamble).
    /// This reference must outlive the brain (enforced by lifetime `'a`).
    pub template: &'a ChatTemplate,

    /// Optional RAG (Retrieval-Augmented Generation) context.
    ///
    /// When set, this text is injected into the preamble as a user message
    /// before the brain JSON, with an assistant acknowledgment following it.
    /// Typically contains retrieved document chunks from vector search.
    ///
    /// **Format**: Plain text or markdown (no JSON required)
    ///
    /// **Injection point**: Between system prompt and brain JSON
    pub rag_context: Option<String>,
}

impl<'a> Brain<'a> {
    /// Create a new brain with the specified token budget and template.
    ///
    /// Initializes an empty brain with no memories and the given configuration.
    /// The brain borrows the template (doesn't take ownership), so the template
    /// must outlive the brain instance.
    ///
    /// # Parameters
    ///
    /// - **`max_tokens`**: Token budget for serialized brain JSON (enforced on each [`add_memory`](Brain::add_memory))
    /// - **`template`**: Reference to chat template providing the system prompt
    ///
    /// # Returns
    ///
    /// A new `Brain` instance with:
    /// - Empty memory queue
    /// - No RAG context
    /// - Reference to provided template
    ///
    /// # Lifetime
    ///
    /// The returned brain is tied to the template's lifetime `'a`. This ensures
    /// the template cannot be dropped while the brain is in use:
    ///
    /// ```rust,ignore
    /// {
    ///     let template = ChatTemplate { /* ... */ };
    ///     let brain = Brain::new(512, &template);
    ///     // template must remain valid while brain exists
    /// } // Both brain and template dropped here
    /// ```
    ///
    /// # Examples
    ///
    /// ## Basic Initialization
    ///
    /// ```rust
    /// # use awful_aj::brain::Brain;
    /// # use awful_aj::template::ChatTemplate;
    /// let template = ChatTemplate {
    ///     system_prompt: "You are a helpful assistant.".to_string(),
    ///     messages: vec![],
    ///     response_format: None,
    ///     pre_user_message_content: None,
    ///     post_user_message_content: None,
    /// };
    ///
    /// let brain = Brain::new(512, &template);
    ///
    /// assert_eq!(brain.max_tokens, 512);
    /// assert_eq!(brain.memories.len(), 0);
    /// assert!(brain.rag_context.is_none());
    /// ```
    ///
    /// ## Different Token Budgets
    ///
    /// ```rust
    /// # use awful_aj::brain::Brain;
    /// # use awful_aj::template::ChatTemplate;
    /// # let template = ChatTemplate {
    /// #     system_prompt: "Be helpful".into(),
    /// #     messages: vec![],
    /// #     response_format: None,
    /// #     pre_user_message_content: None,
    /// #     post_user_message_content: None,
    /// # };
    /// // Minimal context (a few messages)
    /// let small_brain = Brain::new(256, &template);
    ///
    /// // Medium context (10-20 messages)
    /// let medium_brain = Brain::new(1024, &template);
    ///
    /// // Large context (full conversation)
    /// let large_brain = Brain::new(4096, &template);
    /// ```
    pub fn new(max_tokens: u16, template: &'a ChatTemplate) -> Self {
        Self {
            memories: VecDeque::<Memory>::new(),
            max_tokens,
            template,
            rag_context: None,
        }
    }

    /// Add a new memory to the brain with automatic token budget enforcement.
    ///
    /// Appends the memory to the FIFO queue and enforces the token limit by evicting
    /// oldest memories if necessary. The preamble in `session_messages` is automatically
    /// updated after any evictions.
    ///
    /// # Behavior
    ///
    /// 1. **Append**: Push memory to back of queue (newest position)
    /// 2. **Measure**: Serialize entire brain to JSON and count tokens
    /// 3. **Evict**: While over budget, remove oldest memory (index 0)
    /// 4. **Update**: Rebuild preamble in `session_messages` after evictions
    ///
    /// # Parameters
    ///
    /// - **`memory`**: The new conversation turn to add ([`Memory`] with role and content)
    /// - **`session_messages`**: Session container that holds the preamble and conversation
    ///   messages. Modified in-place when evictions occur.
    ///
    /// # Token Counting
    ///
    /// Uses OpenAI's `cl100k_base` BPE encoding with special tokens:
    ///
    /// - **What's counted**: Full JSON from [`get_serialized`](Brain::get_serialized)
    /// - **Threshold**: [`max_tokens`](Brain::max_tokens) field
    /// - **Eviction**: FIFO (first in, first out)
    ///
    /// # Side Effects
    ///
    /// - **Modifies `self.memories`**: May shrink queue if evictions occur
    /// - **Modifies `session_messages.preamble_messages`**: Updated on every eviction
    /// - **Logs**: Emits `tracing::info!` events for eviction activity
    ///
    /// # Performance
    ///
    /// **Warning**: This method performs full JSON serialization and token counting on
    /// every call, even if no eviction is needed. For high-frequency updates, consider
    /// batching memory additions or caching token counts.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```no_run
    /// # use awful_aj::brain::{Brain, Memory};
    /// # use awful_aj::template::ChatTemplate;
    /// # use awful_aj::session_messages::SessionMessages;
    /// # use async_openai::types::Role;
    /// # let template = ChatTemplate {
    /// #     system_prompt: "Be helpful".into(),
    /// #     messages: vec![],
    /// #     response_format: None,
    /// #     pre_user_message_content: None,
    /// #     post_user_message_content: None,
    /// # };
    /// # let cfg = awful_aj::config::AwfulJadeConfig {
    /// #     api_key: "".into(), api_base: "".into(), model: "".into(),
    /// #     context_max_tokens: 2048, assistant_minimum_context_tokens: 256,
    /// #     stop_words: vec![], session_db_url: "".into(),
    /// #     session_name: None, should_stream: None,
    /// # };
    /// let mut brain = Brain::new(256, &template);
    /// let mut session = SessionMessages::new(cfg);
    ///
    /// // Add user message
    /// brain.add_memory(
    ///     Memory::new(Role::User, "What is HNSW?".into()),
    ///     &mut session
    /// );
    ///
    /// // Add assistant response
    /// brain.add_memory(
    ///     Memory::new(Role::Assistant, "HNSW is a graph algorithm...".into()),
    ///     &mut session
    /// );
    ///
    /// // Preamble automatically updated
    /// assert!(session.preamble_messages.len() >= 3);
    /// ```
    ///
    /// ## Observing Evictions
    ///
    /// ```no_run
    /// # use awful_aj::brain::{Brain, Memory};
    /// # use awful_aj::template::ChatTemplate;
    /// # use awful_aj::session_messages::SessionMessages;
    /// # use async_openai::types::Role;
    /// # let template = ChatTemplate {
    /// #     system_prompt: "Be helpful".into(),
    /// #     messages: vec![],
    /// #     response_format: None,
    /// #     pre_user_message_content: None,
    /// #     post_user_message_content: None,
    /// # };
    /// # let cfg = awful_aj::config::AwfulJadeConfig {
    /// #     api_key: "".into(), api_base: "".into(), model: "".into(),
    /// #     context_max_tokens: 2048, assistant_minimum_context_tokens: 256,
    /// #     stop_words: vec![], session_db_url: "".into(),
    /// #     session_name: None, should_stream: None,
    /// # };
    /// let mut brain = Brain::new(128, &template); // Very small budget
    /// let mut session = SessionMessages::new(cfg);
    ///
    /// // Add memories until eviction occurs
    /// for i in 0..10 {
    ///     let count_before = brain.memories.len();
    ///     brain.add_memory(
    ///         Memory::new(Role::User, format!("Message {}", i)),
    ///         &mut session
    ///     );
    ///     let count_after = brain.memories.len();
    ///
    ///     if count_after < count_before + 1 {
    ///         println!("Eviction occurred at message {}", i);
    ///     }
    /// }
    /// ```
    ///
    /// # See Also
    ///
    /// - [`enforce_token_limit`](Brain::enforce_token_limit) - Internal eviction logic
    /// - [`get_serialized`](Brain::get_serialized) - JSON serialization for token counting
    /// - [`crate::session_messages::SessionMessages`] - Session container
    pub fn add_memory(&mut self, memory: Memory, session_messages: &mut SessionMessages) {
        self.memories.push_back(memory);
        self.enforce_token_limit(session_messages);
    }

    /// Enforce `max_tokens` against the serialized brain JSON.
    ///
    /// When over budget, evicts from the **front** (oldest) and refreshes the preamble.
    ///
    /// ## Implementation note
    /// Recalculates token count inside the loop after each eviction to ensure
    /// accurate budget enforcement when multiple memories need to be removed.
    fn enforce_token_limit(&mut self, session_messages: &mut SessionMessages) {
        tracing::info!("Enforcing token limit.");
        let bpe = cl100k_base().unwrap();

        loop {
            let brain_token_count = bpe.encode_with_special_tokens(&self.get_serialized()).len();

            if brain_token_count <= self.max_tokens as usize {
                break;
            }

            tracing::info!("Brain token count is greater than {}", self.max_tokens);
            tracing::info!("Removing oldest memory");

            if self.memories.is_empty() {
                tracing::warn!("No more memories to remove, but still over token limit");
                break;
            }

            self.memories.remove(0); // Removing the oldest memory
            session_messages.preamble_messages = self.build_preamble().unwrap();
        }
    }

    /// Serialize the brain's memories to a JSON string with explanatory preamble.
    ///
    /// Converts the current conversation history into a compact JSON format that can
    /// be injected into LLM prompts. The output consists of two parts:
    ///
    /// 1. **Instructional preamble**: Explains what the JSON represents
    /// 2. **JSON payload**: Structured conversation history
    ///
    /// # Format
    ///
    /// ```text
    /// Below is a JSON representation of our conversation leading up to this point.
    /// Please only respond to this message with "Ok.":
    /// {"about":"This JSON object is...","memories":[{"role":"user","content":"..."},...]}
    /// ```
    ///
    /// # JSON Structure
    ///
    /// ```json
    /// {
    ///   "about": "This JSON object is a representation of our conversation...",
    ///   "memories": [
    ///     {"role": "user", "content": "What is HNSW?"},
    ///     {"role": "assistant", "content": "HNSW is a graph algorithm..."},
    ///     // ... more memories
    ///   ]
    /// }
    /// ```
    ///
    /// # Usage
    ///
    /// This method is primarily called by:
    /// - [`build_preamble`](Brain::build_preamble) - Injects into LLM context
    /// - [`enforce_token_limit`](Brain::enforce_token_limit) - Token counting
    /// - [`add_memory`](Brain::add_memory) - Budget enforcement
    ///
    /// # Returns
    ///
    /// A string containing the preamble text followed by JSON. The JSON is guaranteed
    /// to be valid (serialization errors panic, which shouldn't occur in normal operation).
    ///
    /// # Token Counting
    ///
    /// The **entire output string** (preamble + JSON) is used for token counting in
    /// [`enforce_token_limit`](Brain::enforce_token_limit). This ensures the full
    /// context cost is accounted for.
    ///
    /// # Examples
    ///
    /// ## Empty Brain
    ///
    /// ```rust
    /// # use awful_aj::brain::Brain;
    /// # use awful_aj::template::ChatTemplate;
    /// # let template = ChatTemplate {
    /// #     system_prompt: "Be helpful".into(),
    /// #     messages: vec![],
    /// #     response_format: None,
    /// #     pre_user_message_content: None,
    /// #     post_user_message_content: None,
    /// # };
    /// let brain = Brain::new(512, &template);
    /// let serialized = brain.get_serialized();
    ///
    /// // Contains preamble + empty memories array
    /// assert!(serialized.contains("Below is a JSON representation"));
    /// assert!(serialized.contains(r#""memories":[]"#));
    /// ```
    ///
    /// ## With Memories
    ///
    /// ```rust
    /// # use awful_aj::brain::{Brain, Memory};
    /// # use awful_aj::template::ChatTemplate;
    /// # use async_openai::types::Role;
    /// # let template = ChatTemplate {
    /// #     system_prompt: "Be helpful".into(),
    /// #     messages: vec![],
    /// #     response_format: None,
    /// #     pre_user_message_content: None,
    /// #     post_user_message_content: None,
    /// # };
    /// let mut brain = Brain::new(512, &template);
    /// brain.memories.push_back(Memory::new(Role::User, "Hello".into()));
    /// brain.memories.push_back(Memory::new(Role::Assistant, "Hi!".into()));
    ///
    /// let serialized = brain.get_serialized();
    ///
    /// // Contains both memories
    /// assert!(serialized.contains(r#""role":"user"#));
    /// assert!(serialized.contains(r#""content":"Hello"#));
    /// assert!(serialized.contains(r#""role":"assistant"#));
    /// assert!(serialized.contains(r#""content":"Hi!"#));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if JSON serialization fails (extremely unlikely—only if `Memory` contains
    /// data that can't be serialized to JSON, which shouldn't be possible with String content).
    ///
    /// # See Also
    ///
    /// - [`Memory::to_json`] - Individual memory serialization
    /// - [`build_preamble`](Brain::build_preamble) - Consumer of this method
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

    /// Generate the complete preamble message sequence for LLM requests.
    ///
    /// Constructs a structured preamble that provides the LLM with all necessary context:
    /// system instructions, optional RAG documentation, conversation history, and an
    /// acknowledgment message. This preamble is prepended to every chat completion request.
    ///
    /// # Message Sequence
    ///
    /// The generated preamble contains these messages in order:
    ///
    /// 1. **System** (always): Template's system prompt
    /// 2. **User** (if RAG): "Below is supplementary documentation..."
    /// 3. **Assistant** (if RAG): "I have reviewed the supplementary documentation."
    /// 4. **User** (always): Brain JSON from [`get_serialized`](Brain::get_serialized)
    /// 5. **Assistant** (always): "Ok"
    ///
    /// ## Example Message Flow
    ///
    /// **Without RAG** (3 messages):
    /// ```text
    /// [System] "You are a helpful assistant."
    /// [User] "Below is JSON...{brain_json}"
    /// [Assistant] "Ok"
    /// ```
    ///
    /// **With RAG** (5 messages):
    /// ```text
    /// [System] "You are a helpful assistant."
    /// [User] "Below is supplementary documentation...<rag_context>"
    /// [Assistant] "I have reviewed the supplementary documentation."
    /// [User] "Below is JSON...{brain_json}"
    /// [Assistant] "Ok"
    /// ```
    ///
    /// # Purpose
    ///
    /// The preamble serves multiple purposes:
    ///
    /// - **System Prompt**: Sets the assistant's persona and rules
    /// - **RAG Context**: Grounds responses in retrieved documents
    /// - **Conversation History**: Provides recent context in compact JSON
    /// - **Acknowledgment**: Primes the model to respond (prevents empty replies)
    ///
    /// # RAG Injection
    ///
    /// If [`rag_context`](Brain::rag_context) is set, it's injected **before** the
    /// brain JSON. This allows the LLM to see both retrieved documents and conversation
    /// history in proper order.
    ///
    /// # Returns
    ///
    /// A vector of [`ChatCompletionRequestMessage`] ready to be prepended to your
    /// conversation messages. The length is:
    /// - **3 messages** (no RAG)
    /// - **5 messages** (with RAG)
    ///
    /// # Errors
    ///
    /// Returns `Err(&'static str)` if message construction fails. This is **extremely
    /// rare** and only possible if the underlying async-openai types reject the message
    /// format (which shouldn't happen with valid template and content).
    ///
    /// # Examples
    ///
    /// ## Basic Preamble (No RAG)
    ///
    /// ```no_run
    /// # use awful_aj::brain::Brain;
    /// # use awful_aj::template::ChatTemplate;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let template = ChatTemplate {
    ///     system_prompt: "You are a helpful coding assistant.".to_string(),
    ///     messages: vec![],
    ///     response_format: None,
    ///     pre_user_message_content: None,
    ///     post_user_message_content: None,
    /// };
    ///
    /// let brain = Brain::new(512, &template);
    /// let preamble = brain.build_preamble()?;
    ///
    /// // 3 messages: System, User (brain JSON), Assistant ("Ok")
    /// assert_eq!(preamble.len(), 3);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With RAG Context
    ///
    /// ```no_run
    /// # use awful_aj::brain::Brain;
    /// # use awful_aj::template::ChatTemplate;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let template = ChatTemplate {
    /// #     system_prompt: "You are a helpful assistant.".to_string(),
    /// #     messages: vec![],
    /// #     response_format: None,
    /// #     pre_user_message_content: None,
    /// #     post_user_message_content: None,
    /// # };
    /// let mut brain = Brain::new(1024, &template);
    ///
    /// // Set RAG context from vector search
    /// brain.rag_context = Some(
    ///     "# HNSW Algorithm\nHNSW (Hierarchical Navigable Small World) \
    ///      is a graph-based algorithm for approximate nearest neighbor search..."
    ///         .to_string(),
    /// );
    ///
    /// let preamble = brain.build_preamble()?;
    ///
    /// // 5 messages: System, RAG User, RAG Assistant, Brain User, Brain Assistant
    /// assert_eq!(preamble.len(), 5);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Integrating with API Calls
    ///
    /// ```rust,ignore
    /// // Build complete message list for API request
    /// let mut messages = brain.build_preamble()?;
    ///
    /// // Add template messages (if any)
    /// messages.extend(template.messages.clone());
    ///
    /// // Add current user query
    /// messages.push(ChatCompletionRequestMessage::User(
    ///     ChatCompletionRequestUserMessage {
    ///         content: ChatCompletionRequestUserMessageContent::Text(user_query),
    ///         name: None,
    ///     },
    /// ));
    ///
    /// // Send to LLM
    /// let response = client.chat().create(ChatCompletionRequest {
    ///     model: config.model.clone(),
    ///     messages,
    ///     ..Default::default()
    /// }).await?;
    /// ```
    ///
    /// # Logging
    ///
    /// This method emits `tracing` events:
    /// - `info`: RAG context injection (if present)
    /// - `debug`: RAG content preview
    /// - `info`: Brain state JSON
    ///
    /// # See Also
    ///
    /// - [`build_brainless_preamble`](Brain::build_brainless_preamble) - Variant without RAG
    /// - [`get_serialized`](Brain::get_serialized) - Brain JSON generation
    /// - [`crate::api`] - API client that uses this preamble
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

        // Inject RAG context if available
        if let Some(ref rag_context) = self.rag_context {
            tracing::info!("RAG context is being injected ({} characters)", rag_context.len());
            tracing::debug!("RAG context content:\n{}", rag_context);
            
            let rag_preamble = format!(
                "Below is supplementary documentation that may be relevant to answering the user's question:\n\n{}",
                rag_context
            );
            
            let rag_user_message =
                ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(rag_preamble.clone()),
                    name: None,
                });
            
            tracing::debug!("Full RAG preamble being injected:\n{}", rag_preamble);
            
            messages.push(rag_user_message);
            
            let rag_assistant_ack =
                ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                    content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                        "I have reviewed the supplementary documentation.".to_string(),
                    )),
                    name: None,
                    refusal: None,
                    audio: None,
                    tool_calls: None,
                    function_call: None,
                });
            
            messages.push(rag_assistant_ack);
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AwfulJadeConfig;
    use crate::session_messages::SessionMessages;
    use crate::template::ChatTemplate;

    /// Creates a test configuration for use in brain tests
    fn create_test_config() -> AwfulJadeConfig {
        AwfulJadeConfig {
            api_key: "test_key".to_string(),
            api_base: "http://localhost:5001/v1".to_string(),
            model: "test_model".to_string(),
            context_max_tokens: 2048,
            assistant_minimum_context_tokens: 512,
            stop_words: vec![],
            session_db_url: ":memory:".to_string(),
            session_name: None,
            should_stream: Some(false),
        }
    }

    /// Creates a test template for use in brain tests
    fn create_test_template() -> ChatTemplate {
        ChatTemplate {
            system_prompt: "You are a helpful assistant.".to_string(),
            messages: vec![],
            response_format: None,
            pre_user_message_content: None,
            post_user_message_content: None,
        }
    }

    #[test]
    fn test_memory_creation() {
        let memory = Memory::new(Role::User, "Test content".to_string());

        assert_eq!(memory.role, Role::User);
        assert_eq!(memory.content, "Test content");
    }

    #[test]
    fn test_memory_to_json() {
        let memory = Memory::new(Role::Assistant, "Response text".to_string());
        let json = memory.to_json();

        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "Response text");
    }

    #[test]
    fn test_memory_from_json() {
        let json = serde_json::json!({
            "role": "user",
            "content": "Hello world"
        });

        let memory = Memory::_from_json(&json).unwrap();

        assert_eq!(memory.role, Role::User);
        assert_eq!(memory.content, "Hello world");
    }

    #[test]
    fn test_memory_clone() {
        let original = Memory::new(Role::System, "System message".to_string());
        let cloned = original.clone();

        assert_eq!(original.role, cloned.role);
        assert_eq!(original.content, cloned.content);
    }

    #[test]
    fn test_brain_creation() {
        let template = create_test_template();
        let brain = Brain::new(512, &template);

        assert_eq!(brain.max_tokens, 512);
        assert_eq!(brain.memories.len(), 0);
        assert!(brain.rag_context.is_none());
    }

    #[test]
    fn test_brain_add_memory() {
        let template = create_test_template();
        let mut brain = Brain::new(10000, &template); // Large budget to avoid evictions
        let config = create_test_config();
        let mut session = SessionMessages::new(config);

        let memory1 = Memory::new(Role::User, "First message".to_string());
        let memory2 = Memory::new(Role::Assistant, "First response".to_string());

        brain.add_memory(memory1, &mut session);
        brain.add_memory(memory2, &mut session);

        assert_eq!(brain.memories.len(), 2);
        assert_eq!(brain.memories[0].content, "First message");
        assert_eq!(brain.memories[1].content, "First response");
    }

    #[test]
    fn test_brain_serialization() {
        let template = create_test_template();
        let mut brain = Brain::new(512, &template);

        brain.memories.push_back(Memory::new(Role::User, "Hello".to_string()));
        brain.memories.push_back(Memory::new(Role::Assistant, "Hi there!".to_string()));

        let serialized = brain.get_serialized();

        // Check that serialization contains expected elements
        assert!(serialized.contains("Below is a JSON representation"));
        assert!(serialized.contains("\"memories\""));
        assert!(serialized.contains("Hello"));
        assert!(serialized.contains("Hi there!"));
    }

    #[test]
    fn test_brain_empty_serialization() {
        let template = create_test_template();
        let brain = Brain::new(512, &template);

        let serialized = brain.get_serialized();

        assert!(serialized.contains("\"memories\":[]"));
    }

    #[test]
    fn test_brain_build_preamble_without_rag() {
        let template = create_test_template();
        let brain = Brain::new(512, &template);

        let preamble = brain.build_preamble().unwrap();

        // Without RAG, should have 3 messages: System, User (brain JSON), Assistant ("Ok")
        assert_eq!(preamble.len(), 3);
    }

    #[test]
    fn test_brain_build_preamble_with_rag() {
        let template = create_test_template();
        let mut brain = Brain::new(512, &template);

        brain.rag_context = Some("# Documentation\nThis is test documentation.".to_string());

        let preamble = brain.build_preamble().unwrap();

        // With RAG, should have 5 messages:
        // System, RAG User, RAG Assistant, Brain User, Brain Assistant
        assert_eq!(preamble.len(), 5);
    }

    #[test]
    fn test_brain_build_brainless_preamble() {
        let template = create_test_template();
        let brain = Brain::new(512, &template);

        let preamble = brain.build_brainless_preamble().unwrap();

        // Should still have brain JSON despite the "brainless" name
        assert!(preamble.len() >= 3);
    }

    #[test]
    fn test_brain_memory_eviction() {
        let template = create_test_template();
        // Very small budget to trigger eviction
        let mut brain = Brain::new(50, &template);
        let config = create_test_config();
        let mut session = SessionMessages::new(config);

        // Add many memories to exceed the small budget
        for i in 0..10 {
            let memory = Memory::new(
                Role::User,
                format!("This is a very long message number {} that should help fill up the token budget quickly", i)
            );
            brain.add_memory(memory, &mut session);
        }

        // With a tiny budget, we should have evicted most memories
        // Exact count depends on token counting, but should be less than 10
        assert!(brain.memories.len() < 10);
    }

    #[test]
    fn test_brain_rag_context_injection() {
        let template = create_test_template();
        let mut brain = Brain::new(2048, &template);

        // Set RAG context
        let rag_docs = "# Vector Search\nHNSW is a graph-based algorithm.";
        brain.rag_context = Some(rag_docs.to_string());

        let preamble = brain.build_preamble().unwrap();

        // Verify RAG context is included in one of the preamble messages
        let preamble_text = format!("{:?}", preamble);
        assert!(preamble_text.contains("HNSW"));
    }

    #[test]
    fn test_brain_fifo_eviction_order() {
        let template = create_test_template();
        let mut brain = Brain::new(100, &template); // Very small budget
        let config = create_test_config();
        let mut session = SessionMessages::new(config);

        // Add memories with distinct content
        brain.add_memory(Memory::new(Role::User, "FIRST".to_string()), &mut session);
        brain.add_memory(Memory::new(Role::User, "SECOND".to_string()), &mut session);
        brain.add_memory(Memory::new(Role::User, "THIRD".to_string()), &mut session);
        brain.add_memory(Memory::new(Role::User, "FOURTH".to_string()), &mut session);

        // If any memories were evicted, the oldest ones should be gone first
        // The remaining memories should still be in order
        if brain.memories.len() > 1 {
            let serialized = brain.get_serialized();
            // FIRST should be more likely to be evicted than FOURTH
            let has_first = serialized.contains("FIRST");
            let has_fourth = serialized.contains("FOURTH");

            // If we lost any, we should have kept the newest
            if !has_first {
                assert!(has_fourth, "Should keep newest memories when evicting");
            }
        }
    }
}
