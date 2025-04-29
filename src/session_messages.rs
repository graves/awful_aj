use async_openai::types::{ChatCompletionRequestMessage, Role};
use diesel::{Connection, SqliteConnection};

use crate::{
    config::{establish_connection, AwfulJadeConfig},
    models::{Conversation, Message},
};

use diesel::prelude::*;
use tiktoken_rs::cl100k_base;

pub struct SessionMessages {
    pub preamble_messages: Vec<ChatCompletionRequestMessage>,
    pub conversation_messages: Vec<ChatCompletionRequestMessage>,
    config: AwfulJadeConfig,
    sqlite_connection: SqliteConnection,
}

impl SessionMessages {
    pub fn new(config: AwfulJadeConfig) -> Self {
        Self {
            preamble_messages: Vec::new(),
            conversation_messages: Vec::new(),
            config: config.clone(),
            sqlite_connection: establish_connection(&config.session_db_url),
        }
    }

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

    pub fn persist_message(&mut self, message: &Message) -> Result<Message, diesel::result::Error> {
        let message: Message = self.sqlite_connection.transaction(|conn| {
            diesel::insert_into(crate::schema::messages::table)
                .values(message)
                .returning(Message::as_returning())
                .get_result(conn)
        })?;

        Ok(message)
    }

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

    pub fn insert_message(&mut self, role: String, content: String) -> Result<Message, diesel::result::Error> {
        let conversation = self.query_conversation().unwrap();
        let chat_message = Self::serialize_chat_message(
            role,
            content,
            false,
            &conversation,
        );

        return self.persist_message(&chat_message);
    }

    pub fn query_conversation(&mut self) -> Result<Conversation, diesel::result::Error> {
        let a_session_name = self
            .config
            .session_name
            .as_ref()
            .expect("No session name on AwfulJadeConfig");

        let conversation: Result<Conversation, diesel::result::Error> =
            self.sqlite_connection.transaction(|conn| {
                let existing_conversation: Result<Conversation, diesel::result::Error> =
                    crate::schema::conversations::table
                        .filter(crate::schema::conversations::session_name.eq(a_session_name))
                        .first(conn);

                existing_conversation
            });

        conversation
    }

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

    pub fn string_to_role(role: &String) -> Role {
        match role.as_str() {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            err => panic!("Role in message not allowed: {}", err),
        }
    }

    pub fn count_tokens_in_message(message: &Message) -> isize {
        let bpe = cl100k_base().unwrap();
        let msg_tokens = bpe.encode_with_special_tokens(&message.content);

        msg_tokens.len() as isize
    }

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

    pub fn max_tokens(&self) -> isize {
        ((self.config.context_max_tokens as i32) - self.config.assistant_minimum_context_tokens)
            as isize
    }

    pub fn should_eject_message(&self) -> bool {
        let session_token_count =
            Self::count_tokens_in_chat_completion_messages(&self.preamble_messages)
                + Self::count_tokens_in_chat_completion_messages(&self.conversation_messages);
        tracing::info!("SESSION TOKEN COUNT: {}", session_token_count);
        tracing::info!("ALLOTTED TOKENS {}", self.max_tokens());

        session_token_count > self.max_tokens()
    }
}
