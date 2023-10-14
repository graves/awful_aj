use async_openai::types::{ChatCompletionRequestMessage, Role};
use rust_bert::pipelines::conversation;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::VecDeque;
use std::collections::HashMap;

use crate::template::ChatTemplate;
use crate::vector_store::VectorStore;

#[derive(Debug)]
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

    pub fn from_json(json: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}

pub struct Brain {
    memories: VecDeque<Memory>,
    max_tokens: u16,
    template: &ChatTemplate,
}

impl Brain {
    pub fn new(max_tokens: u16, template: &ChatTemplate) -> Self {
        Self {
            memories: VecDeque::<Memory>::new(),
            max_tokens,
            template
        }
    }

    pub fn add_memory(&mut self, memory: Memory, user_request_message: &ChatCompletionRequestMessage, config: &AwfulJadeConfig) {
        self.memories.push(memory);
        self.enforce_token_limit(&user_request_message, config);
    }

    fn enforce_token_limit(&mut self, user_request_message: &ChatCompletionRequestMessage, config: &AwfulJadeConfig) {
        let conversation = self.build_preamble().expect("Failed to build preamble");
        conversation.push(*user_request_message);

        let token_count = VectorStore::count_tokens(conversation, config);
        if token_count > self.max_tokens {
            while VectorStore::count_tokens(conversation, config) > self.max_tokens 
                && !self.memories.is_empty() {
                self.memories.remove(0);  // Removing the oldest memory
                conversation = self.build_preamble().expect("Failed to build preamble");
                conversation.push(*user_request_message);
            }
        }
    }

    pub fn get_serialized(&self) -> String {
        let about = "This JSON object is a representation of our conversation leading up to this point. This object represents your memories.";
    
        let mut map = HashMap::new();
        map.insert("about", JsonValue::String(about.into()));
        map.insert("memories", JsonValue::Array(self.memories.iter().map(|m| m.to_json()).collect()));
    
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
