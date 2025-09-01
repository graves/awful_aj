## 🧩 Use as a Library

Bring `awful_aj` into your own Rust projects—reuse the same high-level chat plumbing that powers the CLI. 🦀💬

> ⚠️ **Note**: The public API may evolve. Check [docs.rs](https://docs.rs/awful_aj) for signatures.

## 📦 Add Dependency
```toml
# Cargo.toml
[dependencies]
awful_aj = "*"
tokio = "1.45.0"
```

## 🐆 Quickstart

```rust
use awful_aj::{
    api::ask,
    config::AwfulJadeConfig,
    template::{self, ChatTemplate},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config: AwfulJadeConfig = awful_aj::config::load_config("somewhere/config.yaml")
    let template: ChatTemplate = template::load_template("book_txt_sanitizer")
        .await
        .map_err(|e| format!("Template load error: {e}"))?;

    let res = ask(config, chunk.to_string(), template, None, None).await;

    Ok(())
}
```

## 🔎 API Highlights
- `AwfulJadeConfig`: Load/override runtime settings.
- `awful_jade::api::ask(..., None, None)`: One‑shot Q&A.
- `awful_jade::api::ask(..., vector_store, brain)`: Conversations with relevant context injected that falls outside your models maximum context length.
- In-memory vectordb with flat-file persistence, powers `aj`'s memory helpers behind the scenes.


## 🐆 Quickstart: One-Shot Q&A (Non-Streaming)

Uses `api::ask` with no session and no memory. Minimal + predictable. ✅

```rust
use std::error::Error;
use awful_aj::{
    api,
    config::AwfulJadeConfig,
    template::ChatTemplate,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Build config explicitly for clarity. (You can load from your own source if you prefer.)
    let cfg = AwfulJadeConfig {
        api_key: "YOUR_KEY".into(),
        api_base: "http://localhost:5001/v1".into(), // OpenAI-compatible endpoint
        model: "qwen3_30b_a3".into(),
        context_max_tokens: 32768,
        assistant_minimum_context_tokens: 2048,
        stop_words: vec![],                 // forwarded to the request
        session_db_url: "aj.db".into(),     // unused when session_name is None
        session_name: None,                 // no session persistence
        should_stream: Some(false),         // non-streaming
    };

    let tpl = ChatTemplate {
        system_prompt: "You are Qwen, a helpful assistant.".into(),
        messages: vec![],                   // extra seed messages if you want
        response_format: None,              // set to a JSON schema for structured output
        pre_user_message_content: None,     // optional prepend to user input
        post_user_message_content: None,    // optional append to user input
    };

    let answer = api::ask(&cfg, "Hello from my app!".into(), &tpl, None, None).await?;
    println!("assistant: {answer}");

    Ok(())
}
```

What happens under the hood 🧠
- Builds a Client → prepares a preamble (system + template messages).
- Applies `pre_user_message_content` and/or `post_user_message_content`.
- Sends one non-streaming request (because s`hould_stream = Some(false)`).
- Returns the assistant’s text (and persists to DB only if sessions are enabled—see below).

## 📺 Streaming Responses (Live Tokens!)

Set `should_stream = Some(true)` and still call `api::ask(...)`. The tokens print to stdout in blue/bold as they arrive (and you still get the final text returned).

```rust
let mut cfg = /* ... as above ... */ AwfulJadeConfig {
    // ...
    should_stream: Some(true),
    // ...
};
let tpl = /* ... */;
```

📝 Note: The streaming printer uses [crossterm](https://github.com/crossterm-rs/crossterm) for color + attributes. It writes to the locked stdout and resets formatting at the end.

## 🧵 Sessions: Persistent Conversations (with Optional Memory)

Turn on sessions by setting a session_name. When the rolling conversation exceeds the token budget, the oldest user/assistant pair is ejected and (if you provide a VectorStore) embedded + stored for later retrieval. 📚➡️🧠

```rust
use std::error::Error;
use awful_aj::{api, config::AwfulJadeConfig, template::ChatTemplate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cfg = AwfulJadeConfig {
        // ... same as before ...
        session_db_url: "aj.db".into(),
        session_name: Some("getting-started".into()),  // ✅ enable session
        should_stream: Some(false),
        .. /* the rest */
        AwfulJadeConfig {
            api_key: "KEY".into(),
            api_base: "http://localhost:1234/v1".into(),
            model: "jade_qwen3_4b".into(),
            context_max_tokens: 32768,
            assistant_minimum_context_tokens: 2048,
            stop_words: vec![],
            session_db_url: "aj.db".into(),
            session_name: Some("getting-started".into()),
            should_stream: Some(false),
        }
    };

    let tpl = ChatTemplate {
        system_prompt: "You are Awful Jade, created by Awful Security.".into(),
        messages: vec![],
        response_format: None,
        pre_user_message_content: None,
        post_user_message_content: None,
    };

    // First turn
    let a1 = api::ask(&cfg, "Remember: my project is 'Alabaster'.".into(), &tpl, None, None).await?;
    println!("assistant: {a1}");

    // Next turn—session context is restored from DB automatically:
    let a2 = api::ask(&cfg, "What's the codename I told you?".into(), &tpl, None, None).await?;
    println!("assistant: {a2}");

    Ok(())
}
```

Session details 🗂️
- `get_session_messages` loads or seeds the conversation.
- On overflow, the oldest pair is ejected and (if a VectorStore is provided) embedded + persisted; the HNSW index is rebuilt.
- On each call, the assistant reply is persisted to the session DB.

💡 You control the budget with `context_max_tokens` and the preamble budget with `assistant_minimum_context_tokens` (used by the brain/preamble logic).

## 🧠 Adding Memories (Vector Search Assist)

If you provide a `VectorStore` and a `Brain`, nearby memories ([Euclidean distance](https://programminghistorian.org/en/lessons/common-similarity-measures#euclidean-distance) < 1.0) are injected into the brain’s preamble before the call. This is how long-term recall is blended in. 🧲🧠✨

```rust
// RAG demo
use std::{fs, path::PathBuf};
use async_openai::types::Role;
use std::error::Error;
use awful_aj::{api, config::AwfulJadeConfig, template::ChatTemplate, brain::Brain, vector_store::VectorStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cfg = AwfulJadeConfig {
        api_key: "KEY".into(),
        api_base: "http://localhost:5001/v1".into(),
        model: "gpt-4o-mini".into(),
        context_max_tokens: 8192,
        assistant_minimum_context_tokens: 2048,
        stop_words: vec![],
        session_db_url: "aj.db".into(),
        session_name: Some("mem-demo".into()),
        should_stream: Some(false),
    };

    let tpl = ChatTemplate {
        system_prompt: "You are a helpful assistant that uses prior notes when relevant.".into(),
        messages: vec![],
        response_format: None,
        pre_user_message_content: None,
        post_user_message_content: None,
    };

    // Create a brain that will reserve a maximum of 1,024 tokens of the inference's context window.
    let mut brain = Brain::new(1024, &tpl);

    // Construct your VectorStore
    // 384 dims for MiniLM (as per your VectorStore)
    let session_name = "docs-demo";
    let mut store = VectorStore::new(384, session_name.to_string())?;

    // Seed a few memories (they could be doc chunks, FAQs, prior chat turns, etc.)
    let notes = [
        "Project codename is Alabaster.",
        "Primary repository is `awful_aj` owned by `graves`.",
        "Use `aj interactive` for a REPL with memory.",
        "Templates live in the config directory under `templates/`.",
    ];

    for s in notes {
        let v = vs.embed_text_to_vector(s)?;
        vs.add_vector_with_content(v, Memory::new(Role::User, s.to_string()))?;
    }

    // Finalize the HNSW index so queries will see inserts
    store.build()?;

    // Persist both the index (binary) and YAML metadata so you can rehydrate later 💦 (optional)
    let yaml_path = PathBuf::from("vector_store.yaml");
    store.serialize(&yaml_path, session_name.to_string())?;

    // Later, a query that should recall a nearby memory (< 1.0 distance):
    let ans = api::ask(&cfg, "Who owns the repo again?".into(), &tpl, Some(&mut store), Some(&mut brain)).await?;
    println!("assistant: {ans}");

    Ok(())
}
```

What `add_memories_to_brain` does 🔍
1.	Embeds the current question.
2.	Looks up top-k neighbors (3) in the HNSW index.
3.	For neighbors with distance < 1.0, injects their content into the brain.
4.	Rebuilds the preamble so these memories ship with the request.

📏 Threshold and k are implementation details you can tune inside your VectorStore module if you hack on `awful_aj`.

## 🧪 Templates: Powerful Knobs (System, Seeds, & Post-Processing)

`ChatTemplate` gives you flexible pre/post shaping without touching your app logic. 🎛️
- `system_prompt`: The authoritative behavior message.
- `messages`: Seed messages (system/user/assistant) to anchor behavior or provide examples.
- `pre_user_message_content` / `post_user_message_content`: Lightweight way to wrap inputs (e.g., “Answer concisely.” / “Return JSON.”).
- `response_format`: If present, it’s forwarded as a JSON Schema so that if your model supports Tool Calling or Structured Output the inference will only emit structured output. 🧩

🧰 For structured outputs, define the schema object your server expects and place it in `template.response_format`. For example:

```json
{
  "type": "object",
  "properties": {
    "sanitizedBookExcerpt": {
      "type": "string"
    }
  }
}
```

## 🧯 Error Handling (What to Expect)

All public fns bubble up errors: API/network, I/O, (de)serialization, embeddings, DB, index build. Handle with Result<_, Box<dyn Error>> or your own error types.
Streaming writes to stdout can also return I/O errors that are forwarded.

## 🧪 Advanced: Call Streaming/Non-Streaming Primitives Directly

You can skip `api::ask` and call the lower-level primitives if you need full control (custom prompt stacks, different persistence strategy, special output handling):
- `stream_response(...) -> ChatCompletionRequestMessage`
- `fetch_response(...) -> ChatCompletionRequestMessage`

These expect a `Client`, a `SessionMessages` you’ve prepared, and your `AwfulJadeConfig` + `ChatTemplate`. They return the final assistant message object (you extract its text from `AssistantMessageContent::Text`).

⚠️ This is expert-mode: you manage session assembly (`prepare_messages*`) and persistence yourself.

## 🎨 Creative Patterns (Recipes!)

Here are ideas that use only the public API you’ve exposed—copy/paste and riff. 🧑🏽‍🍳

### 1. Batch Q&A (Non-Streaming) 📚⚡

Process a list of prompts and collect answers.
```rust
async fn batch_answer(
    cfg: &awful_aj::config::AwfulJadeConfig,
    tpl: &awful_aj::template::ChatTemplate,
    questions: impl IntoIterator<Item = String>,
) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    for q in questions {
        let a = awful_aj::api::ask(cfg, q, tpl, None, None).await?;
        out.push(a);
    }
    Ok(out)
}
```

### 2. "Sticky" Session Bot 🤝🧵

Keep a named session and call ask repeatedly—great for chat sidebars and Agents.
```rust
struct Sticky<'a> {
    cfg: awful_aj::config::AwfulJadeConfig,
    tpl: awful_aj::template::ChatTemplate,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> Sticky<'a> {
    async fn send(&self, user_text: &str) -> anyhow::Result<String> {
        awful_aj::api::ask(&self.cfg, user_text.into(), &self.tpl, None, None).await.map_err(Into::into)
    }
}
```

### 3. "Context Sandwich" Wrapper 🥪

Standardize how you wrap user input with pre/post content—without changing the template.
```rust
async fn sandwich(
    cfg: &awful_aj::config::AwfulJadeConfig,
    base_tpl: &awful_aj::template::ChatTemplate,
    pre: &str,
    post: &str,
    user: &str,
) -> anyhow::Result<String> {
    let mut tpl = base_tpl.clone();
    tpl.pre_user_message_content = Some(pre.into());
    tpl.post_user_message_content = Some(post.into());
    awful_aj::api::ask(cfg, user.into(), &tpl, None, None).await.map_err(Into::into)
}
```

### 4. Structured Outputs via Schema 🧾✅

Have your template include a JSON schema (set `response_format`) so responses are machine-readable—perfect for pipelines. (Exact schema type depends on your `async_openai` version; set it on `template.response_format` and a`pi::ask` will forward it.)

## 🗺️ Mental Model Recap
= Config (`AwfulJadeConfig`) → client setup + budgets + behavior flags (streaming, sessions).
- Template (`ChatTemplate`) → system, seed messages, schema, pre/post hooks.
- Session (`session_name`) → DB-backed rolling history with ejection on overflow.
- Memory (`VectorStore` + `Brain`) → ejected pairs get embedded; nearest neighbors (`< 1.0`) are injected into the preamble next time.
- Modes → streaming (`should_stream: true`) vs non-streaming (`false/None`).

You choose how many of those dials to turn. 🎛️😄

## ✅ Checklist for Production
- Pin crate versions.
- Decide on streaming vs non-streaming per use-case.
- If you want history, set session_name.
- If you want long-term recall, wire a `VectorStore` and a `Brain`.
- Establish sensible stop words and token budgets.
- Consider a JSON schema when you need structured output.

---

> Happy hacking! 🧩🧠✨