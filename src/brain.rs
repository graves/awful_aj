use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize)]
pub struct Memory {
    content: String,
}

impl Memory {
    pub fn new(content: String) -> Self {
        Self { content }
    }

    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "content": self.content,
        })
    }

    pub fn from_json(json: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}

pub struct Brain {
    memories: VecDeque<Memory>,
    max_memory: usize,
}

impl Brain {
    pub fn new(max_memory: usize) -> Self {
        Self {
            memories: VecDeque::with_capacity(max_memory),
            max_memory,
        }
    }

    pub fn add_memory(&mut self, memory: Memory) {
        if self.memories.len() >= self.max_memory {
            self.memories.pop_front();
        }
        self.memories.push_back(memory);
    }

    pub fn serialize_memories(&self) -> JsonValue {
        let system_message = "This JSON object contains a sequence of content strings, each representing a distinct memory of the assistant.";

        let memories_json: Vec<_> = self
            .memories
            .iter()
            .map(|memory| memory.to_json())
            .collect();

        serde_json::json!({
            "system": system_message,
            "memories": memories_json
        })
    }

    pub fn deserialize_memories(&mut self, json: &JsonValue) -> Result<(), serde_json::Error> {
        if let Some(memories_json) = json.get("memories") {
            let memories: Vec<Memory> = serde_json::from_value(memories_json.clone())?;
            self.memories = memories.into_iter().collect();
        }
        Ok(())
    }
}
