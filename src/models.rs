use diesel::prelude::*;

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

#[derive(Queryable, Identifiable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Conversation {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub session_name: String,
}

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