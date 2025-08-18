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

/// Memory Struct
///
/// Represents a single memory entry in the brain. It holds the role and content of a message.
///
/// # Example
/// ```rust
/// use awful_aj::brain::Memory;
/// use async_openai::types::Role;
///
/// let memory = Memory::new(Role::User, "Hello, how are you?".to_string());
/// assert_eq!(memory.role, Role::User);
/// assert_eq!(memory.content, "Hello, how are you?".to_string());
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Memory {
    /// The role of the memory. Can be User, Assistant, or System.
    pub role: Role,

    /// The content of the message.
    pub content: String,
}

/// Memory Implementation
impl Memory {
    /// Creates a new instance of `Memory`.
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }

    /// Converts the `Memory` instance to JSON format.
    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "role": self.role,
            "content": self.content,
        })
    }

    /// Deserializes JSON into a `Memory` instance.
    pub fn _from_json(json: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}
/// Brain Struct
///
/// Represents the brain of the AI model. It holds a vector deque of memories, a limit on the
/// number of tokens, and a reference to the chat template.
#[derive(Debug)]
pub struct Brain<'a> {
    /// The list of memories in the brain. Kept in a vector deque for efficient push and pop operations.
    pub memories: VecDeque<Memory>,

    /// The maximum number of tokens allowed in the brain.
    pub max_tokens: u16,

    /// A reference to the chat template.
    pub template: &'a ChatTemplate,
}

/// Brain Implementation
impl<'a> Brain<'a> {
    pub fn new(max_tokens: u16, template: &'a ChatTemplate) -> Self {
        Self {
            memories: VecDeque::<Memory>::new(),
            max_tokens,
            template,
        }
    }

    /// Adds a `Memory` instance to the brain. Also enforces the token limit and updates the
    /// preamble messages if necessary.
    pub fn add_memory(&mut self, memory: Memory, session_messages: &mut SessionMessages) {
        self.memories.push_back(memory);
        self.enforce_token_limit(session_messages);
    }

    /// Enforces the token limit in the brain. If the limit is exceeded, it removes memories from
    /// the oldest to the newest until the limit is met. It also updates the preamble messages
    /// if necessary.
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

    /// Converts the brain into JSON format. It includes information about the conversation
    /// memories and is designed to be responded to by the user with an "Ok" message.
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

    /// Builds the preamble messages for the AI model. Includes a system prompt, the brain state as
    /// JSON and an "Ok" message. Used for initialization of further conversations with the AI.
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
