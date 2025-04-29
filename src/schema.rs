// @generated automatically by Diesel CLI.

diesel::table! {
    awful_configs (id) {
        id -> Integer,
        api_base -> Text,
        api_key -> Text,
        model -> Text,
        context_max_tokens -> Integer,
        assistant_minimum_context_tokens -> Integer,
        stop_words -> Text,
        conversation_id -> Nullable<Integer>,
    }
}

diesel::table! {
    conversations (id) {
        id -> Integer,
        session_name -> Text,
    }
}

diesel::table! {
    messages (id) {
        id -> Integer,
        role -> Text,
        content -> Text,
        dynamic -> Bool,
        conversation_id -> Nullable<Integer>,
    }
}

diesel::joinable!(awful_configs -> conversations (conversation_id));
diesel::joinable!(messages -> conversations (conversation_id));

diesel::allow_tables_to_appear_in_same_query!(
    awful_configs,
    conversations,
    messages,
);
