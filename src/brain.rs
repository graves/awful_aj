use async_openai::types::{ChatCompletionRequestMessage, Role};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tiktoken_rs::cl100k_base;
use std::collections::HashMap;
use std::collections::VecDeque;

use crate::session_messages::SessionMessages;
use crate::template::ChatTemplate;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Memory {
    role: Role,
    content: String,
}

impl Memory {
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }

    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "role": self.role,
            "content": self.content,
        })
    }

    pub fn _from_json(json: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}

pub struct Brain<'a> {
    memories: VecDeque<Memory>,
    max_tokens: u16,
    template: &'a ChatTemplate,
}

impl<'a> Brain<'a> {
    pub fn new(max_tokens: u16, template: &'a ChatTemplate) -> Self {
        Self {
            memories: VecDeque::<Memory>::new(),
            max_tokens,
            template,
        }
    }

    pub fn add_memory(&mut self, memory: Memory, session_messages: &mut SessionMessages) {
        self.memories.push_back(memory);
        self.enforce_token_limit(session_messages);
    }

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

    pub fn build_preamble(&self) -> Result<Vec<ChatCompletionRequestMessage>, &'static str> {
        let mut messages: Vec<ChatCompletionRequestMessage> = vec![ChatCompletionRequestMessage {
            role: Role::System,
            content: Some(self.template.system_prompt.clone()),
            name: None,
            function_call: None,
        }];

        let brain_json = self.get_serialized();
        tracing::info!("State of brain: {:?}", brain_json);

        messages.push(ChatCompletionRequestMessage {
            role: Role::User,
            content: Some(brain_json),
            name: None,
            function_call: None,
        });

        messages.push(ChatCompletionRequestMessage {
            role: Role::Assistant,
            content: Some("Ok.".to_string()),
            name: None,
            function_call: None,
        });

        Ok(messages)
    }
}
