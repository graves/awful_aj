-- up.sql
CREATE TABLE awful_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    api_base TEXT NOT NULL,
    api_key TEXT NOT NULL,
    model TEXT NOT NULL,
    context_max_tokens INTEGER NOT NULL,
    assistant_minimum_context_tokens INTEGER NOT NULL,
    stop_words TEXT NOT NULL,
    conversation_id INTEGER,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

CREATE TABLE conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    session_name TEXT NOT NULL UNIQUE
);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    dynamic BOOLEAN NOT NULL DEFAULT true,
    conversation_id INTEGER,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);
